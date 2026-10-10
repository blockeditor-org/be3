mod center;
mod hints;
mod markup;
mod server;

use std::sync::mpsc::Receiver;
use std::time::{SystemTime, UNIX_EPOCH};

use beui::styled::{Toast, ToastAction};
use block_plugin_api::{
    HostNotification, HostNotificationAction, NotificationAction, Notifications as Listed,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use center::{Center, CloseReason, DEFAULT_ACTION, Urgency};
use server::Request;

const TOAST_IDS: u64 = 1 << 32;

pub(crate) struct Notifications {
    requests: Receiver<Request>,
    signals: UnboundedSender<center::Signal>,
    center: Center,
    published: Option<u64>,
}

impl Notifications {
    pub(crate) fn start() -> Self {
        let (sender, requests) = crate::host::waking_channel();
        let (signals, heard) = unbounded_channel();
        crate::dbus::spawn(server::run(sender, heard));
        Self {
            requests,
            signals,
            center: Center::default(),
            published: None,
        }
    }

    pub(crate) fn frame(&mut self, asked: Vec<NotificationAction>) {
        let now = crate::host::now();
        for request in self.requests.try_iter() {
            match request {
                Request::Notify(incoming, reply) => {
                    let id = self.center.notify(*incoming, now, unix_seconds());
                    let _ = reply.send(id);
                }
                Request::Close(id) => {
                    self.center.close(id, CloseReason::Closed);
                }
            }
        }
        for action in asked {
            match action {
                NotificationAction::Invoke { id, action } => {
                    self.center.invoke(id, &action);
                }
                NotificationAction::Dismiss(ids) => {
                    for id in ids {
                        self.center.dismiss(id);
                    }
                }
            }
        }
        if let Some(wait) = self.center.frame(now) {
            crate::host::request_repaint_after(wait);
        }
        for signal in self.center.take_signals() {
            let _ = self.signals.send(signal);
        }
        let revision = self.center.revision();
        if self.published != Some(revision) {
            self.published = Some(revision);
            crate::plugin_host::publish::<Listed>(&self.center.listed().map(listed).collect());
        }
    }

    pub(crate) fn toasts(&self, locked: bool) -> Vec<Toast> {
        toasts(&self.center, locked)
    }

    pub(crate) fn dismiss_toast(&mut self, toast: u64) {
        if let Some(id) = notification(toast) {
            self.center.dismiss(id);
        }
    }

    pub(crate) fn invoke_toast(&mut self, toast: u64, action: &str) {
        if let Some(id) = notification(toast) {
            self.center.invoke(id, action);
        }
    }
}

pub(crate) fn activate_action() -> &'static str {
    DEFAULT_ACTION
}

fn notification(toast: u64) -> Option<u32> {
    toast
        .checked_sub(TOAST_IDS)
        .and_then(|id| u32::try_from(id).ok())
}

fn listed(kept: &center::Notification) -> HostNotification {
    HostNotification {
        id: kept.id,
        app_name: kept.incoming.app_name.clone(),
        summary: kept.incoming.summary.clone(),
        body: kept.incoming.body.clone(),
        received: kept.received,
        critical: kept.incoming.urgency == Urgency::Critical,
        actions: kept
            .incoming
            .actions
            .iter()
            .map(|action| HostNotificationAction {
                key: action.key.clone(),
                label: action.label.clone(),
            })
            .collect(),
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn toasts(center: &Center, locked: bool) -> Vec<Toast> {
    if locked {
        return Vec::new();
    }
    center
        .toasts()
        .map(|kept| Toast {
            id: TOAST_IDS + u64::from(kept.id),
            title: kept.incoming.summary.clone(),
            message: kept.incoming.body.clone(),
            danger: kept.incoming.urgency == Urgency::Critical && kept.incoming.image.is_none(),
            image: kept.incoming.image.clone(),
            actions: kept
                .incoming
                .actions
                .iter()
                .filter(|action| action.key != DEFAULT_ACTION)
                .map(|action| ToastAction {
                    key: action.key.clone(),
                    label: action.label.clone(),
                })
                .collect(),
            activates: kept.has_default_action(),
            sticky: true,
        })
        .collect()
}

#[cfg(test)]
mod tests;
