use std::future::pending;
use std::sync::mpsc::Receiver;

use futures_util::future::{Either, select};
use futures_util::{StreamExt, pin_mut};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::Proxy;
use zbus::proxy::SignalStream;
use zbus::zvariant::{OwnedFd, OwnedObjectPath};

use super::power::{LogindAbilities, LogindCall};
use crate::host::{WakingSender, waking_channel};

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
    sender: WakingSender<LogindEvent>,
    events: Receiver<LogindEvent>,
}

impl Logind {
    pub(crate) fn connect() -> Self {
        let (requests, requested) = unbounded_channel();
        let (sender, events) = waking_channel();
        crate::dbus::spawn(run(requested, sender.clone()));
        Self {
            requests,
            sender,
            events,
        }
    }

    pub(crate) fn call(&self, call: LogindCall) {
        if self.requests.send(Request::Call(call)).is_err() {
            let _ = self.sender.send(LogindEvent::Called(
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

    pub(crate) fn events(&self) -> Vec<LogindEvent> {
        self.events.try_iter().collect()
    }
}

enum Next {
    Request(Option<Request>),
    Sleep(Option<bool>),
    Lock(Option<()>),
}

async fn run(mut requested: UnboundedReceiver<Request>, events: WakingSender<LogindEvent>) {
    let Some(connection) = crate::dbus::system().await else {
        return;
    };
    let manager = match Proxy::new(&connection, DESTINATION, PATH, MANAGER).await {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("block-app: logind is not reachable: {error}");
            return;
        }
    };
    let abilities = LogindAbilities {
        suspend: can(&manager, "CanSuspend").await,
        reboot: can(&manager, "CanReboot").await,
        power_off: can(&manager, "CanPowerOff").await,
    };
    let _ = events.send(LogindEvent::Abilities(abilities));
    let mut sleeps = match manager.receive_signal("PrepareForSleep").await {
        Ok(sleeps) => Some(sleeps),
        Err(error) => {
            eprintln!("block-app: logind's PrepareForSleep cannot be watched: {error}");
            None
        }
    };
    let session = own_session(&manager).await;
    let mut locks = match &session {
        Some(session) => match session.receive_signal("Lock").await {
            Ok(locks) => Some(locks),
            Err(error) => {
                eprintln!("block-app: logind's Lock cannot be watched: {error}");
                None
            }
        },
        None => None,
    };
    let mut inhibitor: Option<OwnedFd> = None;
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
                let _ = events.send(LogindEvent::Called(call, result));
            }
            Next::Request(Some(Request::Inhibit)) => {
                if inhibitor.is_none() {
                    inhibitor = inhibit(&manager).await;
                }
            }
            Next::Request(Some(Request::Release)) => inhibitor = None,
            Next::Request(Some(Request::LockedHint(locked))) => {
                if let Some(session) = &session
                    && let Err(error) = session.call::<_, _, ()>("SetLockedHint", &(locked,)).await
                {
                    eprintln!("block-app: logind did not take the locked hint: {error}");
                }
            }
            Next::Request(None) => return,
            Next::Sleep(Some(starting)) => {
                let _ = events.send(LogindEvent::PrepareForSleep(starting));
            }
            Next::Sleep(None) => sleeps = None,
            Next::Lock(Some(())) => {
                let _ = events.send(LogindEvent::Lock);
            }
            Next::Lock(None) => locks = None,
        }
    }
}

async fn own_session<'a>(manager: &Proxy<'a>) -> Option<Proxy<'a>> {
    let path = match manager
        .call::<_, _, OwnedObjectPath>("GetSessionByPID", &(std::process::id(),))
        .await
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("block-app: this process is in no logind session: {error}");
            return None;
        }
    };
    match Proxy::new(manager.connection(), DESTINATION, path, SESSION).await {
        Ok(session) => Some(session),
        Err(error) => {
            eprintln!("block-app: logind's session is not reachable: {error}");
            None
        }
    }
}

async fn inhibit(manager: &Proxy<'_>) -> Option<OwnedFd> {
    let asked = manager
        .call::<_, _, OwnedFd>(
            "Inhibit",
            &("sleep", "Block", "Lock the screen before suspending", "delay"),
        )
        .await;
    match asked {
        Ok(inhibitor) => Some(inhibitor),
        Err(error) => {
            eprintln!("block-app: logind did not let the screen lock before suspending: {error}");
            None
        }
    }
}

async fn can(manager: &Proxy<'_>, method: &str) -> bool {
    match manager.call::<_, _, String>(method, &()).await {
        Ok(answer) => answer == "yes",
        Err(error) => {
            eprintln!("block-app: logind did not answer {method}: {error}");
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
                eprintln!("block-app: logind's PrepareForSleep could not be read: {error}");
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
