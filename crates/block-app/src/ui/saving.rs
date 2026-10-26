use std::time::Duration;

use beui::reactive::*;
use beui::styled::*;

use super::AppViewStore;

pub(super) const SLOW_SAVE: Duration = Duration::from_secs(4);
pub(super) const TOAST_ID: u64 = u64::MAX;

#[derive(Clone, Copy)]
pub(super) struct SlowSave {
    shown: Memo<bool>,
    dismissed: Signal<bool>,
}

impl SlowSave {
    pub(super) fn new(view: AppViewStore) -> Self {
        let status = view.status;
        let saved = create_memo(move || status.get().sync != super::SyncState::Syncing);
        let saves = create_memo(move || status.get().saves);
        let slow = create_signal(false);
        let dismissed = create_signal(false);
        let waiting = create_timer(move || {
            slow.set(true);
            None
        });
        create_effect(move || {
            saves.get();
            match saved.get() {
                true => untrack(|| {
                    waiting.stop();
                    slow.set(false);
                    dismissed.set(false);
                }),
                false => untrack(|| {
                    if !slow.get() {
                        waiting.restart(SLOW_SAVE);
                    }
                }),
            }
        });
        let shown = create_memo(move || slow.get() && !dismissed.get());
        Self { shown, dismissed }
    }

    pub(super) fn dismiss(self) {
        self.dismissed.set(true);
    }

    pub(super) fn with_toast(self, mut toasts: Vec<Toast>) -> Vec<Toast> {
        if self.shown.get() {
            toasts.insert(
                0,
                Toast {
                    id: TOAST_ID,
                    title: "Saving changes…".to_owned(),
                    message:
                        "This is taking longer than usual. Keep the app open until it finishes."
                            .to_owned(),
                    sticky: true,
                    ..Toast::default()
                },
            );
        }
        toasts
    }
}

#[cfg(test)]
mod tests;
