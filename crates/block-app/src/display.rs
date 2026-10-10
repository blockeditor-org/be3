use std::cell::RefCell;
use std::time::Duration;

use be_block::DisplaySettings;
use be_block::be_model::Edit;

use crate::app_state::{AppStateError, AppStateStore};
use crate::local_settings::LocalSettings;

mod guard;

pub(crate) use guard::{ANSWER_WITHIN, Prompt, Screen};
use guard::Guard;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::apply;
#[cfg(target_os = "linux")]
pub(crate) use linux::{start, stop};

#[cfg(not(target_os = "linux"))]
pub(crate) fn start(_setup: &beui::Setup) {}

#[cfg(not(target_os = "linux"))]
pub(crate) fn stop() {}

#[cfg(not(target_os = "linux"))]
fn apply(_settings: &DisplaySettings) {}

thread_local! {
    static GUARD: RefCell<Guard> = RefCell::new(Guard::default());
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn set_screens(screens: Vec<Screen>) {
    GUARD.with(|guard| guard.borrow_mut().set_screens(screens));
}

pub(crate) fn asking() -> bool {
    GUARD.with(|guard| guard.borrow().asking())
}

pub(crate) fn prompt(now: Duration) -> Prompt {
    GUARD.with(|guard| guard.borrow_mut().prompt(now))
}

pub(crate) fn keep() -> Vec<Edit> {
    settle(Guard::keep)
}

pub(crate) fn revert() -> Vec<Edit> {
    settle(Guard::revert)
}

fn settle(answer: impl FnOnce(&mut Guard) -> Vec<Edit>) -> Vec<Edit> {
    let (edits, applied) = GUARD.with(|guard| {
        let mut guard = guard.borrow_mut();
        let edits = answer(&mut guard);
        (edits, guard.applied().clone())
    });
    apply(&applied);
    edits
}

impl LocalSettings for DisplaySettings {
    const NAME: &'static str = "display settings";

    fn load(store: &AppStateStore) -> Result<Option<Vec<u8>>, AppStateError> {
        store.display_settings()
    }

    fn save(store: &AppStateStore, content: &[u8]) -> Result<(), AppStateError> {
        store.set_display_settings(content)
    }

    fn apply(&self) -> bool {
        let settled = GUARD.with(|guard| guard.borrow_mut().take(self.clone()));
        apply(self);
        settled
    }
}
