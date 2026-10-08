use std::future::pending;
use std::sync::mpsc::Receiver;

use futures_util::future::{Either, select};
use futures_util::{StreamExt, pin_mut};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::Proxy;
use zbus::proxy::SignalStream;

use super::power::{LogindAbilities, LogindCall};
use crate::host::{WakingSender, waking_channel};

const DESTINATION: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";

#[derive(Debug)]
pub(crate) enum LogindEvent {
    Abilities(LogindAbilities),
    Called(LogindCall, Result<(), String>),
    PrepareForSleep(bool),
}

pub(crate) struct Logind {
    calls: UnboundedSender<LogindCall>,
    sender: WakingSender<LogindEvent>,
    events: Receiver<LogindEvent>,
}

impl Logind {
    pub(crate) fn connect() -> Self {
        let (calls, requested) = unbounded_channel();
        let (sender, events) = waking_channel();
        crate::dbus::spawn(run(requested, sender.clone()));
        Self {
            calls,
            sender,
            events,
        }
    }

    pub(crate) fn call(&self, call: LogindCall) {
        if self.calls.send(call).is_err() {
            let _ = self.sender.send(LogindEvent::Called(
                call,
                Err("logind is not reachable".to_owned()),
            ));
        }
    }

    pub(crate) fn events(&self) -> Vec<LogindEvent> {
        self.events.try_iter().collect()
    }
}

async fn run(mut requested: UnboundedReceiver<LogindCall>, events: WakingSender<LogindEvent>) {
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
    loop {
        let next = {
            let call = requested.recv();
            let sleep = next_sleep(&mut sleeps);
            pin_mut!(call, sleep);
            match select(call, sleep).await {
                Either::Left((call, _)) => Either::Left(call),
                Either::Right((sleep, _)) => Either::Right(sleep),
            }
        };
        match next {
            Either::Left(Some(call)) => {
                let result = manager
                    .call::<_, _, ()>(call.method(), &(false,))
                    .await
                    .map_err(|error| error.to_string());
                let _ = events.send(LogindEvent::Called(call, result));
            }
            Either::Left(None) => return,
            Either::Right(Some(starting)) => {
                let _ = events.send(LogindEvent::PrepareForSleep(starting));
            }
            Either::Right(None) => sleeps = None,
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
