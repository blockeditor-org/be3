use std::future::{Future, pending};
use std::ops::ControlFlow;

use futures_util::future::{Either, select};
use futures_util::{StreamExt, pin_mut};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::proxy::SignalStream;
use zbus::zvariant::{OwnedFd, OwnedObjectPath};
use zbus::{Connection, Proxy};

use super::power::{LogindAbilities, LogindCall};
use be_host_base::host::Post;

const DESTINATION: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";

#[derive(Debug)]
pub(crate) enum LogindEvent {
    Abilities(LogindAbilities),
    Called(LogindCall, Result<(), String>),
    PrepareForSleep(bool),
    Lock,
}

#[derive(Clone, Copy, Debug)]
enum Request {
    Call(LogindCall),
    Inhibit,
    Release,
    LockedHint(bool),
}

pub(crate) struct Logind {
    requests: UnboundedSender<Request>,
    events: Post<LogindEvent>,
}

impl Logind {
    pub(crate) fn connect(events: Post<LogindEvent>) -> Self {
        let (requests, requested) = unbounded_channel();
        be_host_services::dbus::spawn(run(requested, events.clone()));
        Self { requests, events }
    }

    pub(crate) fn call(&self, call: LogindCall) {
        if self.requests.send(Request::Call(call)).is_err() {
            self.events.send(LogindEvent::Called(
                call,
                Err("logind is not reachable".to_owned()),
            ));
        }
    }

    pub(crate) fn inhibit_sleep(&self) {
        let _ = self.requests.send(Request::Inhibit);
    }

    pub(crate) fn release_sleep(&self) {
        let _ = self.requests.send(Request::Release);
    }

    pub(crate) fn set_locked_hint(&self, locked: bool) {
        let _ = self.requests.send(Request::LockedHint(locked));
    }
}

enum Next {
    Request(Option<Request>),
    Sleep(Option<bool>),
    Lock(Option<()>),
}

#[derive(Default)]
struct Held {
    inhibitor: Option<OwnedFd>,
    inhibiting: bool,
    locked_hint: Option<bool>,
}

async fn run(mut requested: UnboundedReceiver<Request>, events: Post<LogindEvent>) {
    let bus = &be_host_services::dbus::SYSTEM;
    let mut held = Held::default();
    loop {
        let Some(connection) = offline(&mut requested, &events, &mut held, bus.connected()).await
        else {
            return;
        };
        let flow = {
            let serving = online(&connection, &mut requested, &events, &mut held);
            let closed = connection.closed();
            pin_mut!(serving, closed);
            match select(serving, closed).await {
                Either::Left((flow, _)) => flow,
                Either::Right(((), _)) => ControlFlow::Continue(()),
            }
        };
        if flow.is_break() {
            return;
        }
        let closed = connection.closed();
        if offline(&mut requested, &events, &mut held, closed)
            .await
            .is_none()
        {
            return;
        }
    }
}

async fn offline<T>(
    requested: &mut UnboundedReceiver<Request>,
    events: &Post<LogindEvent>,
    held: &mut Held,
    until: impl Future<Output = T>,
) -> Option<T> {
    pin_mut!(until);
    loop {
        let request = requested.recv();
        pin_mut!(request);
        match select(until.as_mut(), request).await {
            Either::Left((value, _)) => return Some(value),
            Either::Right((None, _)) => return None,
            Either::Right((Some(request), _)) => match request {
                Request::Call(call) => events.send(LogindEvent::Called(
                    call,
                    Err("logind is not reachable".to_owned()),
                )),
                Request::Inhibit => held.inhibiting = true,
                Request::Release => {
                    held.inhibiting = false;
                    held.inhibitor = None;
                }
                Request::LockedHint(locked) => held.locked_hint = Some(locked),
            },
        }
    }
}

async fn online(
    connection: &Connection,
    requested: &mut UnboundedReceiver<Request>,
    events: &Post<LogindEvent>,
    held: &mut Held,
) -> ControlFlow<()> {
    let manager = match Proxy::new(connection, DESTINATION, PATH, MANAGER).await {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("be-host: logind is not reachable: {error}");
            return ControlFlow::Continue(());
        }
    };
    let abilities = LogindAbilities {
        suspend: can(&manager, "CanSuspend").await,
        reboot: can(&manager, "CanReboot").await,
        power_off: can(&manager, "CanPowerOff").await,
    };
    events.send(LogindEvent::Abilities(abilities));
    let mut sleeps = match manager.receive_signal("PrepareForSleep").await {
        Ok(sleeps) => Some(sleeps),
        Err(error) => {
            eprintln!("be-host: logind's PrepareForSleep cannot be watched: {error}");
            None
        }
    };
    let session = own_session(&manager).await;
    let mut locks = match &session {
        Some(session) => match session.receive_signal("Lock").await {
            Ok(locks) => Some(locks),
            Err(error) => {
                eprintln!("be-host: logind's Lock cannot be watched: {error}");
                None
            }
        },
        None => None,
    };
    if held.inhibiting && held.inhibitor.is_none() {
        held.inhibitor = inhibit(&manager).await;
    }
    if let (Some(session), Some(locked)) = (&session, held.locked_hint) {
        set_locked_hint(session, locked).await;
    }
    loop {
        let next = {
            let request = requested.recv();
            let sleep = next_sleep(&mut sleeps);
            let lock = next_signal(&mut locks);
            pin_mut!(request, sleep, lock);
            match select(request, select(sleep, lock)).await {
                Either::Left((request, _)) => Next::Request(request),
                Either::Right((Either::Left((sleep, _)), _)) => Next::Sleep(sleep),
                Either::Right((Either::Right((lock, _)), _)) => Next::Lock(lock),
            }
        };
        match next {
            Next::Request(Some(Request::Call(call))) => {
                let result = manager
                    .call::<_, _, ()>(call.method(), &(false,))
                    .await
                    .map_err(|error| error.to_string());
                events.send(LogindEvent::Called(call, result));
            }
            Next::Request(Some(Request::Inhibit)) => {
                held.inhibiting = true;
                if held.inhibitor.is_none() {
                    held.inhibitor = inhibit(&manager).await;
                }
            }
            Next::Request(Some(Request::Release)) => {
                held.inhibiting = false;
                held.inhibitor = None;
            }
            Next::Request(Some(Request::LockedHint(locked))) => {
                held.locked_hint = Some(locked);
                if let Some(session) = &session {
                    set_locked_hint(session, locked).await;
                }
            }
            Next::Request(None) => return ControlFlow::Break(()),
            Next::Sleep(Some(starting)) => {
                events.send(LogindEvent::PrepareForSleep(starting));
            }
            Next::Sleep(None) => sleeps = None,
            Next::Lock(Some(())) => {
                events.send(LogindEvent::Lock);
            }
            Next::Lock(None) => locks = None,
        }
    }
}

async fn set_locked_hint(session: &Proxy<'_>, locked: bool) {
    if let Err(error) = session.call::<_, _, ()>("SetLockedHint", &(locked,)).await {
        eprintln!("be-host: logind did not take the locked hint: {error}");
    }
}

async fn own_session<'a>(manager: &Proxy<'a>) -> Option<Proxy<'a>> {
    let path = match manager
        .call::<_, _, OwnedObjectPath>("GetSessionByPID", &(std::process::id(),))
        .await
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("be-host: this process is in no logind session: {error}");
            return None;
        }
    };
    match Proxy::new(manager.connection(), DESTINATION, path, SESSION).await {
        Ok(session) => Some(session),
        Err(error) => {
            eprintln!("be-host: logind's session is not reachable: {error}");
            None
        }
    }
}

async fn inhibit(manager: &Proxy<'_>) -> Option<OwnedFd> {
    let asked = manager
        .call::<_, _, OwnedFd>(
            "Inhibit",
            &(
                "sleep",
                "Block",
                "Lock the screen before suspending",
                "delay",
            ),
        )
        .await;
    match asked {
        Ok(inhibitor) => Some(inhibitor),
        Err(error) => {
            eprintln!("be-host: logind did not let the screen lock before suspending: {error}");
            None
        }
    }
}

async fn can(manager: &Proxy<'_>, method: &str) -> bool {
    match manager.call::<_, _, String>(method, &()).await {
        Ok(answer) => answer == "yes",
        Err(error) => {
            eprintln!("be-host: logind did not answer {method}: {error}");
            false
        }
    }
}

async fn next_sleep(sleeps: &mut Option<SignalStream<'_>>) -> Option<bool> {
    let Some(stream) = sleeps else {
        return pending().await;
    };
    loop {
        let message = stream.next().await?;
        match message.body().deserialize::<bool>() {
            Ok(starting) => return Some(starting),
            Err(error) => {
                eprintln!("be-host: logind's PrepareForSleep could not be read: {error}");
            }
        }
    }
}

async fn next_signal(signals: &mut Option<SignalStream<'_>>) -> Option<()> {
    let Some(stream) = signals else {
        return pending().await;
    };
    stream.next().await.map(|_| ())
}
