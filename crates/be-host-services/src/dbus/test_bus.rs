use std::future::Future;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use futures_util::future::{Either, select};
use futures_util::{StreamExt, pin_mut};
use zbus::names::BusName;
use zbus::{Connection, MatchRule, MessageStream};

pub const DEADLINE: Duration = Duration::from_secs(30);

pub struct PrivateBus {
    daemon: Option<Child>,
    dir: PathBuf,
    address: String,
}

impl PrivateBus {
    pub fn start() -> Self {
        let dir = std::env::temp_dir().join(format!("be3-bus-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("a directory for the bus");
        let socket = dir.join("socket");
        std::fs::write(
            dir.join("bus.conf"),
            format!(
                "<busconfig><type>session</type><listen>unix:path={}</listen>\
                 <auth>EXTERNAL</auth><policy context=\"default\">\
                 <allow send_destination=\"*\" eavesdrop=\"true\"/><allow eavesdrop=\"true\"/>\
                 <allow own=\"*\"/></policy></busconfig>",
                socket.display()
            ),
        )
        .expect("the bus configuration is written");
        let mut bus = Self {
            daemon: None,
            address: format!("unix:path={}", socket.display()),
            dir,
        };
        bus.start_again();
        bus
    }

    pub fn start_again(&mut self) {
        assert!(self.daemon.is_none(), "the bus is already running");
        let program = std::env::var("BE3_DBUS_DAEMON").unwrap_or_else(|_| "dbus-daemon".to_owned());
        let mut daemon = Command::new(program)
            .arg(format!(
                "--config-file={}",
                self.dir.join("bus.conf").display()
            ))
            .args(["--nofork", "--nopidfile", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon runs");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().expect("the daemon's output"))
            .read_line(&mut address)
            .expect("the daemon prints its address once it listens");
        self.daemon = Some(daemon);
    }

    pub fn kill(&mut self) {
        if let Some(mut daemon) = self.daemon.take() {
            let _ = daemon.kill();
            let _ = daemon.wait();
        }
    }

    pub fn builder(&self) -> zbus::connection::Builder<'_> {
        zbus::connection::Builder::address(self.address.as_str()).expect("the address parses")
    }

    pub async fn connect(&self) -> Connection {
        within(
            "the bus takes a connection",
            crate::dbus::build(self.builder()),
        )
        .await
        .expect("the bus takes a connection")
    }

    pub fn shared(&self) -> Arc<crate::dbus::Bus> {
        Arc::new(crate::dbus::Bus::new(crate::dbus::Source::Address(
            self.address.clone(),
        )))
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        self.kill();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub fn next<T>(receiver: &Receiver<T>, what: &str) -> T {
    match receiver.recv_timeout(DEADLINE) {
        Ok(value) => value,
        Err(error) => panic!("{what} did not happen within {DEADLINE:?}: {error}"),
    }
}

pub async fn within<T>(what: &str, waiting: impl Future<Output = T>) -> T {
    let deadline = async_io::Timer::after(DEADLINE);
    pin_mut!(waiting);
    match select(waiting, deadline).await {
        Either::Left((value, _)) => value,
        Either::Right(_) => panic!("{what} did not happen within {DEADLINE:?}"),
    }
}

pub async fn owned(connection: &Connection, name: &str) {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.DBus")
        .expect("a sender")
        .member("NameOwnerChanged")
        .expect("a member")
        .arg(0, name)
        .expect("an argument")
        .build();
    let mut changes = MessageStream::for_match_rule(rule, connection, None)
        .await
        .expect("the bus takes a match rule");
    let bus = zbus::fdo::DBusProxy::new(connection)
        .await
        .expect("the bus is reachable");
    let name = BusName::try_from(name).expect("a bus name");
    within(&format!("{name} is owned"), async {
        while !bus
            .name_has_owner(name.clone())
            .await
            .expect("the bus answers")
        {
            changes.next().await;
        }
    })
    .await;
}
