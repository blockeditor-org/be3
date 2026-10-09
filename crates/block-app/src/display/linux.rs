use std::cell::RefCell;

use be_block::DisplaySettings;
use be_block::display_settings::DisplayMode as SavedMode;
use beui_adapter_drm::{DisplayConfig, DisplayControl, DisplayMode, Monitor, Screens};
use block_plugin_api::{HostDisplay, HostDisplayMode};

use super::Screen;

thread_local! {
    static CONTROL: RefCell<Option<DisplayControl>> = const { RefCell::new(None) };
    static SCREENS: RefCell<Option<Screens>> = const { RefCell::new(None) };
}

pub(crate) fn start(setup: &beui::Setup) {
    let control = setup.get::<DisplayControl>().cloned();
    if let Some(control) = &control {
        control.on_monitors(|monitors| {
            crate::plugin_host::set_displays(monitors.iter().map(host_display).collect());
            super::set_screens(monitors.iter().map(screen).collect());
            crate::host::wake();
        });
    }
    CONTROL.with(|slot| *slot.borrow_mut() = control);
    let screens = setup.get::<Screens>().cloned();
    SCREENS.with(|slot| *slot.borrow_mut() = screens);
}

pub(crate) fn stop() {
    let control = CONTROL.with(|slot| slot.borrow_mut().take());
    let screens = SCREENS.with(|slot| slot.borrow_mut().take());
    drop((control, screens));
}

pub(crate) fn screens() -> Vec<beui::Rect> {
    SCREENS.with(|screens| {
        screens
            .borrow()
            .as_ref()
            .map(Screens::rects)
            .unwrap_or_default()
    })
}

pub(crate) fn apply(settings: &DisplaySettings) {
    CONTROL.with(|control| {
        if let Some(control) = control.borrow().as_ref() {
            control.set(config(settings));
        }
    });
    crate::wayland::set_blank_after(settings.screen_off().after());
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
