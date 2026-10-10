use std::time::Duration;

use block_editor_beui::beui::Image;
use block_editor_beui::{
    IncomingNotification, NotificationCloseReason, HostImage, NotificationSignal,
    NotificationUrgency,
};

use super::markup;

pub(crate) const DEFAULT_ACTION: &str = "default";
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const MAX_KEPT: usize = 100;
const MAX_BODY: usize = 8 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Action {
    pub(crate) key: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Incoming {
    pub(crate) app_name: String,
    pub(crate) summary: String,
    pub(crate) body: String,
    pub(crate) actions: Vec<Action>,
    pub(crate) urgency: NotificationUrgency,
    pub(crate) image: Option<Image>,
    pub(crate) transient: bool,
    pub(crate) resident: bool,
    pub(crate) expire_timeout: i32,
}

impl Incoming {
    pub(crate) fn from_host(incoming: &IncomingNotification) -> Self {
        Self {
            app_name: incoming.app_name.clone(),
            summary: incoming.summary.clone(),
            body: markup::clip(&markup::plain(&incoming.body), MAX_BODY),
            actions: incoming
                .actions
                .iter()
                .map(|action| Action {
                    key: action.key.clone(),
                    label: action.label.clone(),
                })
                .collect(),
            urgency: incoming.urgency,
            image: incoming.image.as_ref().and_then(picture),
            transient: incoming.transient,
            resident: incoming.resident,
            expire_timeout: incoming.expire_timeout,
        }
    }
}

fn picture(image: &HostImage) -> Option<Image> {
    let pixels = u64::from(image.width) * u64::from(image.height) * 4;
    let fits = image.width > 0 && image.height > 0 && pixels == image.rgba.len() as u64;
    fits.then(|| Image::from_rgba(image.width, image.height, image.rgba.clone()))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Notification {
    pub(crate) id: u32,
    pub(crate) incoming: Incoming,
    pub(crate) received: u64,
    pub(crate) toast: Toast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Toast {
    Hidden,
    Until(Duration),
    Sticky,
}

impl Notification {
    pub(crate) fn toasted(&self) -> bool {
        self.toast != Toast::Hidden
    }

    pub(crate) fn critical(&self) -> bool {
        self.incoming.urgency == NotificationUrgency::Critical
    }

    pub(crate) fn has_default_action(&self) -> bool {
        self.incoming
            .actions
            .iter()
            .any(|action| action.key == DEFAULT_ACTION)
    }
}

#[derive(Default)]
pub(crate) struct Center {
    kept: Vec<Notification>,
    signals: Vec<NotificationSignal>,
    revision: u64,
}

impl Center {
    pub(crate) fn notify(&mut self, id: u32, incoming: Incoming, now: Duration, received: u64) {
        let toast = toast_for(&incoming, now);
        self.revision += 1;
        if let Some(kept) = self.kept.iter_mut().find(|kept| kept.id == id) {
            kept.incoming = incoming;
            kept.received = received;
            kept.toast = toast;
            return;
        }
        self.kept.push(Notification {
            id,
            incoming,
            received,
            toast,
        });
        while self.kept.len() > MAX_KEPT {
            let oldest = self.kept.remove(0);
            self.signals.push(NotificationSignal::Closed(
                oldest.id,
                NotificationCloseReason::Expired,
            ));
        }
    }

    pub(crate) fn close(&mut self, id: u32, reason: NotificationCloseReason) -> bool {
        let Some(at) = self.kept.iter().position(|kept| kept.id == id) else {
            return false;
        };
        self.kept.remove(at);
        self.signals.push(NotificationSignal::Closed(id, reason));
        self.revision += 1;
        true
    }

    pub(crate) fn dismiss(&mut self, id: u32) -> bool {
        self.close(id, NotificationCloseReason::Dismissed)
    }

    pub(crate) fn invoke(&mut self, id: u32, key: &str) -> bool {
        let Some(kept) = self.kept.iter_mut().find(|kept| kept.id == id) else {
            return false;
        };
        if !kept.incoming.actions.iter().any(|action| action.key == key) {
            return false;
        }
        self.signals
            .push(NotificationSignal::ActionInvoked(id, key.to_owned()));
        match kept.incoming.resident {
            true => {
                kept.toast = Toast::Hidden;
                self.revision += 1;
            }
            false => {
                self.close(id, NotificationCloseReason::Dismissed);
            }
        }
        true
    }

    pub(crate) fn frame(&mut self, now: Duration) -> Option<Duration> {
        let mut expired = Vec::new();
        let mut next: Option<Duration> = None;
        for kept in &mut self.kept {
            let Toast::Until(until) = kept.toast else {
                continue;
            };
            if until > now {
                let wait = until - now;
                next = Some(next.map_or(wait, |next| next.min(wait)));
                continue;
            }
            kept.toast = Toast::Hidden;
            self.revision += 1;
            if kept.incoming.transient {
                expired.push(kept.id);
            }
        }
        for id in expired {
            self.close(id, NotificationCloseReason::Expired);
        }
        next
    }

    pub(crate) fn toasts(&self) -> impl Iterator<Item = &Notification> {
        self.kept.iter().filter(|kept| kept.toasted())
    }

    pub(crate) fn listed(&self) -> impl Iterator<Item = &Notification> {
        self.kept
            .iter()
            .rev()
            .filter(|kept| !kept.incoming.transient)
    }

    pub(crate) fn kept_ids(&self) -> Vec<u32> {
        self.kept.iter().map(|kept| kept.id).collect()
    }

    pub(crate) fn take_signals(&mut self) -> Vec<NotificationSignal> {
        std::mem::take(&mut self.signals)
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
}

fn toast_for(incoming: &Incoming, now: Duration) -> Toast {
    if incoming.urgency == NotificationUrgency::Critical {
        return Toast::Sticky;
    }
    match incoming.expire_timeout {
        0 => Toast::Sticky,
        timeout if timeout < 0 => Toast::Until(now + DEFAULT_TIMEOUT),
        timeout => Toast::Until(now + Duration::from_millis(timeout.unsigned_abs().into())),
    }
}

#[cfg(test)]
mod tests;
