use std::collections::HashSet;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, PoisonError};

use block_plugin_api::{
    NotificationCloseReason, NotificationInbox, NotificationReport, NotificationRequest,
    NotificationSignal,
};

pub(crate) const MAX_PENDING: usize = 64;

#[derive(Default)]
pub(crate) struct Ids {
    last: u32,
    in_use: HashSet<u32>,
}

impl Ids {
    pub(crate) fn assign(&mut self, replaces: u32) -> u32 {
        if replaces != 0 && self.in_use.contains(&replaces) {
            return replaces;
        }
        loop {
            self.last = self.last.wrapping_add(1);
            if self.last != 0 && self.in_use.insert(self.last) {
                return self.last;
            }
        }
    }
}

pub(crate) struct Inbox {
    ids: Arc<Mutex<Ids>>,
    pending: Vec<(u64, NotificationRequest)>,
    next: u64,
    kept: Vec<u32>,
}

impl Inbox {
    pub(crate) fn new(ids: Arc<Mutex<Ids>>) -> Self {
        Self {
            ids,
            pending: Vec::new(),
            next: 0,
            kept: Vec::new(),
        }
    }

    pub(crate) fn frame(
        &mut self,
        requests: &Receiver<NotificationRequest>,
        reports: Vec<NotificationReport>,
    ) -> (bool, Vec<NotificationSignal>) {
        let mut ids = self.ids.lock().unwrap_or_else(PoisonError::into_inner);
        let mut changed = false;
        let mut signals = Vec::new();
        for request in requests.try_iter() {
            self.next += 1;
            self.pending.push((self.next, request));
            changed = true;
        }
        let over = self.pending.len().saturating_sub(MAX_PENDING);
        for (_, dropped) in self.pending.drain(..over) {
            if let NotificationRequest::Notify(incoming) = dropped {
                signals.push(NotificationSignal::Closed(
                    incoming.id,
                    NotificationCloseReason::Expired,
                ));
            }
        }
        for report in reports {
            if let Some(received) = report.received {
                let before = self.pending.len();
                self.pending.retain(|(sequence, _)| *sequence > received);
                changed |= self.pending.len() != before;
            }
            signals.extend(report.signals);
            self.kept = report.kept;
        }
        ids.in_use = self
            .kept
            .iter()
            .copied()
            .chain(
                self.pending
                    .iter()
                    .filter_map(|(_, request)| match request {
                        NotificationRequest::Notify(incoming) => Some(incoming.id),
                        NotificationRequest::Close(_) => None,
                    }),
            )
            .collect();
        (changed, signals)
    }

    pub(crate) fn published(&self) -> NotificationInbox {
        NotificationInbox {
            requests: self.pending.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
