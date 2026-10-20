use std::cell::RefCell;

use be_block::DisplaySettings;
use be_block::display_settings::Rotation as SavedRotation;
use be_block::display_settings::DisplayMode as SavedMode;
use beui_adapter_drm::{DisplayConfig, DisplayControl, DisplayMode, Epaper, Monitor, Rotation};
use block_plugin_api::{EpaperRequest, HostDisplay, HostDisplayMode, HostRotation};

use super::Screen;

thread_local! {
    static CONTROL: RefCell<Option<DisplayControl>> = const { RefCell::new(None) };
    static EPAPER: RefCell<Option<Epaper>> = const { RefCell::new(None) };
}

pub(crate) fn start(setup: &beui::Setup) {
    let control = setup.get::<DisplayControl>().cloned();
    if let Some(control) = &control {
        control.on_monitors(|monitors| {
            crate::plugin_host::publish::<block_plugin_api::Displays>(
                &monitors.iter().map(host_display).collect(),
            );
            super::set_screens(monitors.iter().map(screen).collect());
            crate::host::wake();
        });
    }
    CONTROL.with(|slot| *slot.borrow_mut() = control);
    let epaper = setup.get::<Epaper>().cloned();
    if epaper.is_some() {
        crate::plugin_host::publish::<block_plugin_api::Epaper>(&true);
    }
    EPAPER.with(|slot| *slot.borrow_mut() = epaper);
}

pub(crate) fn stop() {
    let control = CONTROL.with(|slot| slot.borrow_mut().take());
    drop(control);
    let epaper = EPAPER.with(|slot| slot.borrow_mut().take());
    drop(epaper);
}

pub(crate) fn run_epaper() {
    let requests = crate::plugin_host::take_actions::<EpaperRequest>();
    if requests.is_empty() {
        return;
    }
    EPAPER.with(|epaper| {
        if let Some(epaper) = epaper.borrow().as_ref() {
            epaper.refresh();
        }
    });
}

pub(crate) fn apply(settings: &DisplaySettings) {
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
            .screens
            .iter()
            .filter_map(|(id, screen)| Some((id.clone(), seat_mode(screen.mode?))))
            .collect(),
        rotations: settings
            .screens
            .iter()
            .filter_map(|(id, screen)| Some((id.clone(), seat_rotation(screen.rotation?))))
            .collect(),
    }
}

fn seat_rotation(rotation: SavedRotation) -> Rotation {
    match rotation {
        SavedRotation::Normal => Rotation::Normal,
        SavedRotation::Clockwise90 => Rotation::Clockwise90,
        SavedRotation::Clockwise180 => Rotation::Clockwise180,
        SavedRotation::Clockwise270 => Rotation::Clockwise270,
    }
}

fn host_rotation(rotation: Rotation) -> HostRotation {
    match rotation {
        Rotation::Normal => HostRotation::Normal,
        Rotation::Clockwise90 => HostRotation::Clockwise90,
        Rotation::Clockwise180 => HostRotation::Clockwise180,
        Rotation::Clockwise270 => HostRotation::Clockwise270,
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
        rotation: host_rotation(monitor.rotation),
    }
}
