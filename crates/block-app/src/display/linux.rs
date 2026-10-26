use std::cell::RefCell;

use be_block::DisplaySettings;
use beui_platform_drm::{DisplayConfig, DisplayControl, Epaper, Monitor};
use block_plugin_api::HostDisplay;

use super::Screen;

thread_local! {
    static CONTROL: RefCell<Option<DisplayControl>> = const { RefCell::new(None) };
    static EPAPER: RefCell<Option<Epaper>> = const { RefCell::new(None) };
}

pub(crate) fn start(setup: &beui::Setup) {
    let control = setup.get::<DisplayControl>().cloned();
    if let Some(control) = &control {
        control.on_monitors(|monitors| {
            crate::plugin_host::publish::<block_plugin_api::DisplaysWatch>(
                &monitors.iter().map(host_display).collect(),
            );
            super::set_screens(monitors.iter().map(screen).collect());
            crate::host::wake();
        });
    }
    CONTROL.with(|slot| *slot.borrow_mut() = control);
    let epaper = setup.get::<Epaper>().cloned();
    if epaper.is_some() {
        crate::plugin_host::publish::<block_plugin_api::EpaperWatch>(&true);
    }
    EPAPER.with(|slot| *slot.borrow_mut() = epaper);
}

pub(crate) fn epaper() -> bool {
    EPAPER.with(|slot| slot.borrow().is_some())
}

pub(crate) fn stop() {
    let control = CONTROL.with(|slot| slot.borrow_mut().take());
    drop(control);
    let epaper = EPAPER.with(|slot| slot.borrow_mut().take());
    drop(epaper);
}

pub(crate) fn run_epaper() {
    let requests = crate::plugin_host::take_calls(block_plugin_api::requested::epaper_refresh);
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
            .filter_map(|(id, screen)| Some((id.clone(), screen.mode?)))
            .collect(),
        rotations: settings
            .screens
            .iter()
            .filter_map(|(id, screen)| Some((id.clone(), screen.rotation?)))
            .collect(),
    }
}

fn screen(monitor: &Monitor) -> Screen {
    Screen {
        id: monitor.id.clone(),
        modes: monitor.modes.clone(),
        default: monitor.default,
        preferred: monitor.preferred,
    }
}

fn host_display(monitor: &Monitor) -> HostDisplay {
    HostDisplay {
        id: monitor.id.clone(),
        name: monitor.name.clone(),
        connector: monitor.connector.clone(),
        modes: monitor.modes.clone(),
        default: monitor.default,
        current: monitor.current,
        rotation: monitor.rotation,
    }
}
