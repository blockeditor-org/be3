use std::cell::RefCell;
use std::time::Duration;

use be_block::be_model::Edit;
use be_block::display_settings::DisplayMode;
use be_block::{DisplaySettings, SettingsLayer};

use crate::app_state::{AppStateError, AppStateStore};
use crate::local_settings::{LayerContents, LocalSettings};

mod guard;
pub(crate) mod shown;

pub(crate) use guard::{ANSWER_WITHIN, Prompt, Screen};
use guard::{Guard, ModeChange, written};
pub(crate) use shown::{apply as apply_shown, create_handle, owner, shown};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::apply;
#[cfg(target_os = "linux")]
pub(crate) use linux::{epaper, run_epaper, start, stop};

#[cfg(not(target_os = "linux"))]
pub(crate) fn start(_setup: &beui::Setup) {}

#[cfg(not(target_os = "linux"))]
pub(crate) fn epaper() -> bool {
    false
}

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

#[cfg(test)]
pub(crate) fn asking() -> bool {
    GUARD.with(|guard| guard.borrow().asking())
}

pub(crate) fn prompt(now: Duration) -> Prompt {
    GUARD.with(|guard| guard.borrow_mut().prompt(now))
}

pub(crate) fn boot(store: &AppStateStore) {
    let tried = match store.tried_display_modes() {
        Ok(Some(text)) => serde_json::from_str::<Vec<(String, DisplayMode)>>(&text)
            .map(|tried| tried.into_iter().collect())
            .unwrap_or_default(),
        Ok(None) => Default::default(),
        Err(error) => {
            eprintln!("block-app: the display modes tried here were not read: {error}");
            Default::default()
        }
    };
    GUARD.with(|guard| guard.borrow_mut().remember(tried));
}

pub(crate) fn keep(
    store: &AppStateStore,
    layers: &LayerContents<DisplaySettings>,
) -> Vec<(SettingsLayer, Edit)> {
    settle(store, layers, Guard::keep)
}

pub(crate) fn revert(
    store: &AppStateStore,
    layers: &LayerContents<DisplaySettings>,
) -> Vec<(SettingsLayer, Edit)> {
    settle(store, layers, Guard::revert)
}

fn settle(
    store: &AppStateStore,
    layers: &LayerContents<DisplaySettings>,
    answer: impl FnOnce(&mut Guard) -> Vec<ModeChange>,
) -> Vec<(SettingsLayer, Edit)> {
    let (changes, applied, tried) = GUARD.with(|guard| {
        let mut guard = guard.borrow_mut();
        let changes = answer(&mut guard);
        (changes, guard.applied().clone(), guard.tried().clone())
    });
    apply(&applied);
    let tried: Vec<(String, DisplayMode)> = tried.into_iter().collect();
    match serde_json::to_string(&tried) {
        Ok(text) => {
            if let Err(error) = store.set_tried_display_modes(&text) {
                eprintln!("block-app: the display modes tried here were not saved: {error}");
            }
        }
        Err(error) => eprintln!("block-app: the display modes tried here were not saved: {error}"),
    }
    written(layers, &changes)
}

impl LocalSettings for DisplaySettings {
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
