mod hints;
mod inbox;
mod server;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use block_app_base::host::{self, Mailbox};
use block_plugin_api::{
    NotificationReport, NotificationRequest, NotificationSignal, NotificationsWatch,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use inbox::{Ids, Inbox};

pub struct Notifications {
    held: Rc<RefCell<Held>>,
    _requests: Mailbox,
}

struct Held {
    signals: UnboundedSender<NotificationSignal>,
    inbox: Inbox,
    offered: bool,
}

impl Notifications {
    pub fn start() -> Self {
        let (signals, heard) = unbounded_channel();
        let ids = Arc::new(Mutex::new(Ids::default()));
        let held = Rc::new(RefCell::new(Held {
            signals,
            inbox: Inbox::new(Arc::clone(&ids)),
            offered: false,
        }));
        let receiving = Rc::clone(&held);
        let (requests, post) = host::mailbox(move |request: NotificationRequest| {
            receiving.borrow_mut().step(vec![request], Vec::new());
        });
        crate::dbus::spawn(server::run(post, heard, ids));
        Self {
            held,
            _requests: requests,
        }
    }

    pub fn frame(&mut self) {
        let reports =
            block_app_base::watches::take_calls(block_plugin_api::requested::notification_report);
        self.held.borrow_mut().step(Vec::new(), reports);
    }
}

impl Held {
    fn step(&mut self, requests: Vec<NotificationRequest>, reports: Vec<NotificationReport>) {
        let (changed, signals) = self.inbox.frame(requests, reports);
        for signal in signals {
            let _ = self.signals.send(signal);
        }
        if changed || !std::mem::replace(&mut self.offered, true) {
            block_app_base::watches::publish::<NotificationsWatch>(&self.inbox.published());
        }
    }
}
