use std::time::Duration;

use beui::Image;

pub(crate) const DEFAULT_ACTION: &str = "default";
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const MAX_KEPT: usize = 100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseReason {
    Expired = 1,
    Dismissed = 2,
    Closed = 3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Action {
    pub(crate) key: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Incoming {
    pub(crate) app_name: String,
    pub(crate) replaces_id: u32,
    pub(crate) summary: String,
    pub(crate) body: String,
    pub(crate) actions: Vec<Action>,
    pub(crate) urgency: Urgency,
    pub(crate) image: Option<Image>,
    pub(crate) transient: bool,
    pub(crate) resident: bool,
    pub(crate) expire_timeout: i32,
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

    pub(crate) fn has_default_action(&self) -> bool {
        self.incoming
            .actions
            .iter()
            .any(|action| action.key == DEFAULT_ACTION)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Signal {
    Closed(u32, CloseReason),
    ActionInvoked(u32, String),
}

#[derive(Default)]
pub(crate) struct Center {
    last_id: u32,
    kept: Vec<Notification>,
    signals: Vec<Signal>,
    revision: u64,
}

impl Center {
    pub(crate) fn notify(&mut self, incoming: Incoming, now: Duration, received: u64) -> u32 {
        let toast = toast_for(&incoming, now);
        self.revision += 1;
        let replaced = incoming.replaces_id;
        if replaced != 0
            && let Some(kept) = self.kept.iter_mut().find(|kept| kept.id == replaced)
        {
            kept.incoming = incoming;
            kept.received = received;
            kept.toast = toast;
            return replaced;
        }
        let id = self.next_id();
        self.kept.push(Notification {
            id,
            incoming,
            received,
            toast,
        });
        while self.kept.len() > MAX_KEPT {
            let oldest = self.kept.remove(0);
            self.signals
                .push(Signal::Closed(oldest.id, CloseReason::Expired));
        }
        id
    }

    fn next_id(&mut self) -> u32 {
        loop {
            self.last_id = self.last_id.wrapping_add(1);
            let id = self.last_id;
            if id != 0 && self.kept.iter().all(|kept| kept.id != id) {
                return id;
            }
        }
    }

    pub(crate) fn close(&mut self, id: u32, reason: CloseReason) -> bool {
        let Some(at) = self.kept.iter().position(|kept| kept.id == id) else {
            return false;
        };
        self.kept.remove(at);
        self.signals.push(Signal::Closed(id, reason));
        self.revision += 1;
        true
    }

    pub(crate) fn dismiss(&mut self, id: u32) -> bool {
        self.close(id, CloseReason::Dismissed)
    }

    pub(crate) fn invoke(&mut self, id: u32, key: &str) -> bool {
        let Some(kept) = self.kept.iter_mut().find(|kept| kept.id == id) else {
            return false;
        };
        if !kept.incoming.actions.iter().any(|action| action.key == key) {
            return false;
        }
        self.signals.push(Signal::ActionInvoked(id, key.to_owned()));
        match kept.incoming.resident {
            true => {
                kept.toast = Toast::Hidden;
                self.revision += 1;
            }
            false => {
                self.close(id, CloseReason::Dismissed);
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
            self.close(id, CloseReason::Expired);
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

    pub(crate) fn take_signals(&mut self) -> Vec<Signal> {
        std::mem::take(&mut self.signals)
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
}

fn toast_for(incoming: &Incoming, now: Duration) -> Toast {
    if incoming.urgency == Urgency::Critical {
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
