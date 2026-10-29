use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zbus::Connection;

pub struct Bus {
    daemon: Child,
    dir: PathBuf,
    address: String,
}

impl Bus {
    pub fn start() -> Self {
        let dir = std::env::temp_dir().join(format!("be3-bus-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("a directory for the bus");
        let config = dir.join("bus.conf");
        std::fs::write(
            &config,
            format!(
                "<busconfig><type>session</type><listen>unix:path={}</listen>\
                 <auth>EXTERNAL</auth><policy context=\"default\">\
                 <allow send_destination=\"*\" eavesdrop=\"true\"/><allow eavesdrop=\"true\"/>\
                 <allow own=\"*\"/></policy></busconfig>",
                dir.join("socket").display()
            ),
        )
        .expect("the bus configuration is written");
        let program = std::env::var("BE3_DBUS_DAEMON").unwrap_or_else(|_| "dbus-daemon".to_owned());
        let mut daemon = Command::new(program)
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--nopidfile", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon runs");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().expect("the daemon's output"))
            .read_line(&mut address)
            .expect("the daemon prints its address once it listens");
        Self {
            daemon,
            dir,
            address: address.trim().to_owned(),
        }
    }

    pub async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .expect("the address parses")
            .build()
            .await
            .expect("the bus takes a connection")
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
