use std::cell::RefCell;
use std::time::Duration;

use be_block::DisplaySettings;
use be_block::be_model::Edit;
use be_block::display_settings::DisplayMode as SavedMode;
use beui_adapter_drm::{DisplayConfig, DisplayControl, DisplayMode, Monitor};
use block_plugin_api::{HostDisplay, HostDisplayMode};

use crate::local_settings::LocalSettings;

mod guard;

use guard::Guard;
pub(crate) use guard::{ANSWER_WITHIN, Prompt, Screen};

thread_local! {
    static CONTROL: RefCell<Option<DisplayControl>> = const { RefCell::new(None) };
    static GUARD: RefCell<Guard> = RefCell::new(Guard::default());
}

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

    fn apply(&self) -> bool {
        let settled = GUARD.with(|guard| guard.borrow_mut().take(self.clone()));
        apply(self);
        settled
    }
}

pub(crate) fn start(setup: &beui::Setup) {
    let control = setup.get::<DisplayControl>().cloned();
    if let Some(control) = &control {
        control.on_monitors(|monitors| {
            block_app_lib::extension::publish::<block_plugin_api::Displays>(
                &monitors.iter().map(host_display).collect(),
            );
            set_screens(monitors.iter().map(screen).collect());
            block_app_lib::extension::wake();
        });
    }
    CONTROL.with(|slot| *slot.borrow_mut() = control);
}

pub(crate) fn stop() {
    let control = CONTROL.with(|slot| slot.borrow_mut().take());
    drop(control);
}

fn apply(settings: &DisplaySettings) {
    CONTROL.with(|control| {
        if let Some(control) = control.borrow().as_ref() {
            control.set(config(settings));
        }
    });
    crate::wayland::set_blank_after(settings.screen_off().after());
    crate::wayland::set_lock_after(settings.lock_time());
}

fn config(settings: &DisplaySettings) -> DisplayConfig {
    DisplayConfig {
        modes: settings
            .monitors
            .iter()
            .filter_map(|(id, monitor)| Some((id.clone(), seat_mode(monitor.mode?))))
            .collect(),
    }
}

fn seat_mode(mode: SavedMode) -> DisplayMode {
    DisplayMode {
        width: mode.width,
        height: mode.height,
        refresh_millihertz: mode.refresh_millihertz,
    }
}

fn saved_mode(mode: DisplayMode) -> SavedMode {
    SavedMode {
        width: mode.width,
        height: mode.height,
        refresh_millihertz: mode.refresh_millihertz,
    }
}

fn screen(monitor: &Monitor) -> Screen {
    Screen {
        id: monitor.id.clone(),
        modes: monitor.modes.iter().copied().map(saved_mode).collect(),
        default: saved_mode(monitor.default),
        preferred: saved_mode(monitor.preferred),
    }
}

fn host_mode(mode: DisplayMode) -> HostDisplayMode {
    HostDisplayMode {
        width: mode.width,
        height: mode.height,
        refresh_millihertz: mode.refresh_millihertz,
    }
}

fn host_display(monitor: &Monitor) -> HostDisplay {
    HostDisplay {
        id: monitor.id.clone(),
        name: monitor.name.clone(),
        connector: monitor.connector.clone(),
        modes: monitor.modes.iter().copied().map(host_mode).collect(),
        default: host_mode(monitor.default),
        current: host_mode(monitor.current),
    }
}
