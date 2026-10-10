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
        let (sender, requests) = crate::host::waking_channel();
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
        let reports = crate::plugin_host::take_actions::<NotificationReport>();
        let (changed, signals) = self.inbox.frame(&self.requests, reports);
        for signal in signals {
            let _ = self.signals.send(signal);
        }
        if changed {
            crate::plugin_host::publish::<block_plugin_api::Notifications>(&self.inbox.published());
        }
    }
}
