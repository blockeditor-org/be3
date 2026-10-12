mod hints;
mod inbox;
mod server;

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use block_plugin_api::{NotificationReport, NotificationRequest, NotificationSignal};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use inbox::{Ids, Inbox};

pub(crate) struct Notifications {
    requests: Receiver<NotificationRequest>,
    signals: UnboundedSender<NotificationSignal>,
    inbox: Inbox,
}

impl Notifications {
    pub(crate) fn start() -> Self {
        let (sender, requests) = block_app_lib::extension::waking_channel();
        let (signals, heard) = unbounded_channel();
        let ids = Arc::new(Mutex::new(Ids::default()));
        crate::dbus::spawn(server::run(sender, heard, Arc::clone(&ids)));
        Self {
            requests,
            signals,
            inbox: Inbox::new(ids),
        }
    }

    pub(crate) fn frame(&mut self) {
        let reports = block_app_lib::extension::take_actions::<NotificationReport>();
        let (changed, signals) = self.inbox.frame(&self.requests, reports);
        for signal in signals {
            let _ = self.signals.send(signal);
        }
        if changed {
            block_app_lib::extension::publish::<block_plugin_api::Notifications>(
                &self.inbox.published(),
            );
        }
    }
}
