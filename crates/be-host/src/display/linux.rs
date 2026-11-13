use std::cell::RefCell;

use be_block::DisplaySettings;
use be_block::display_settings::ScreenUse;
use be_host_desktop::desktop_host::{DesktopHost, Host};
use be_sdk_api::{HostDisplay, ScreenInputState, ScreenRegionState, ScreenSide, ScreenWallState};
use beui_platform_drm::{
    Display, DisplayConfig, DisplayControl, DisplayUse, Displays, Epaper, Monitor, Region,
    ScreenInput, Side, Wall,
};

use super::Screen;
use super::shown::ShownDisplay;

thread_local! {
    static CONTROL: RefCell<Option<DisplayControl>> = const { RefCell::new(None) };
    static EPAPER: RefCell<Option<Epaper>> = const { RefCell::new(None) };
    static MONITORS: RefCell<Vec<Monitor>> = const { RefCell::new(Vec::new()) };
    static DISPLAYS: RefCell<Vec<Display>> = const { RefCell::new(Vec::new()) };
    static SCREEN_INPUT: RefCell<Option<ScreenInput>> = const { RefCell::new(None) };
}

pub(crate) fn start(setup: &beui::native::Setup) {
    let control = setup.get::<DisplayControl>().cloned();
    if let Some(control) = &control {
        control.on_monitors(|monitors| {
            MONITORS.with(|held| *held.borrow_mut() = monitors.to_vec());
            publish();
            super::set_screens(monitors.iter().map(screen).collect());
            be_host_base::host::wake();
        });
    }
    if let Some(displays) = setup.get::<Displays>().cloned() {
        let watched = displays.clone();
        let follow = move || {
            let shown = watched.displays();
            super::shown::offer(
                shown
                    .iter()
                    .map(|display| ShownDisplay {
                        id: display.id.clone(),
                        name: display.name.clone(),
                        rect: display.rect(),
                        scale: display.scale,
                        placement: watched.placement(&display.id),
                    })
                    .collect(),
            );
            DISPLAYS.with(|held| *held.borrow_mut() = shown);
            publish();
        };
        follow();
        displays.watch(follow);
    }
    CONTROL.with(|slot| *slot.borrow_mut() = control);
    let epaper = setup.get::<Epaper>().cloned();
    if epaper.is_some() {
        be_host_base::watches::publish::<be_sdk_api::EpaperWatch>(&true);
    }
    EPAPER.with(|slot| *slot.borrow_mut() = epaper);
    let input = setup.get::<ScreenInput>().cloned();
    if let Some(input) = &input {
        let watched = input.clone();
        let publish = move || {
            let state = screen_input_state(watched.state());
            be_host_base::watches::publish::<be_sdk_api::ScreenInputWatch>(&state);
            be_host_base::host::wake();
        };
        publish();
        input.watch(publish);
    }
    SCREEN_INPUT.with(|slot| *slot.borrow_mut() = input);
}

pub(crate) fn epaper() -> bool {
    EPAPER.with(|slot| slot.borrow().is_some())
}

pub(crate) fn stop() {
    let control = CONTROL.with(|slot| slot.borrow_mut().take());
    drop(control);
    let epaper = EPAPER.with(|slot| slot.borrow_mut().take());
    drop(epaper);
    let input = SCREEN_INPUT.with(|slot| slot.borrow_mut().take());
    drop(input);
}

/// Hands the shell's walls and regions to the platform.
pub(crate) fn run_screen_input() {
    let Some(input) =
        be_host_base::watches::take_calls(be_sdk_api::requested::set_screen_input).pop()
    else {
        return;
    };
    let walls = input
        .walls
        .into_iter()
        .map(|wall| Wall {
            id: wall.id,
            display: wall.display,
            side: match wall.side {
                ScreenSide::Top => Side::Top,
                ScreenSide::Bottom => Side::Bottom,
                ScreenSide::Left => Side::Left,
                ScreenSide::Right => Side::Right,
            },
            start: wall.start,
            end: wall.end,
        })
        .collect();
    let regions = input
        .regions
        .into_iter()
        .map(|region| Region {
            id: region.id,
            rect: region.rect,
            captures_touch: region.captures_touch,
        })
        .collect();
    SCREEN_INPUT.with(|slot| {
        if let Some(held) = slot.borrow().as_ref() {
            held.set(walls, regions);
        }
    });
}

fn screen_input_state(state: beui_platform_drm::ScreenInputState) -> ScreenInputState {
    ScreenInputState {
        walls: state
            .walls
            .into_iter()
            .map(|wall| ScreenWallState {
                id: wall.id,
                against: wall.against,
                pushed: wall.pushed,
            })
            .collect(),
        regions: state
            .regions
            .into_iter()
            .map(|region| ScreenRegionState {
                id: region.id,
                pointer: region.pointer,
                touch_downs: region.touch_downs,
                touch: region.touch,
                lifts: region.lifts,
                lifted: region.lifted,
            })
            .collect(),
        touch_downs: state.touch_downs,
    }
}

pub(crate) fn run_epaper() {
    let requests = be_host_base::watches::take_calls(be_sdk_api::requested::epaper_refresh);
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
    Host::set_blank_after(settings.screen_off().after());
    Host::set_lock_after(settings.lock_time());
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
        scales: settings
            .screens
            .iter()
            .filter_map(|(id, _)| Some((id.clone(), settings.screen_scale(id)?)))
            .collect(),
        uses: settings
            .screens
            .iter()
            .filter_map(|(id, screen)| Some((id.clone(), display_use(screen.used.clone()?))))
            .collect(),
        main: settings.main_screen(),
        places: settings
            .arrangement()
            .places
            .iter()
            .map(|place| (place.id.clone(), beui::pos2(place.x as f32, place.y as f32)))
            .collect(),
    }
}

fn display_use(used: ScreenUse) -> DisplayUse {
    match used {
        ScreenUse::Desktop => DisplayUse::Desktop,
        ScreenUse::Mirror { of } => DisplayUse::Mirror(of),
        ScreenUse::Off => DisplayUse::Off,
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

fn publish() {
    let displays = MONITORS.with(|monitors| {
        DISPLAYS.with(|shown| {
            let shown = shown.borrow();
            monitors
                .borrow()
                .iter()
                .map(|monitor| {
                    host_display(
                        monitor,
                        shown.iter().find(|display| display.id == monitor.id),
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    be_host_base::watches::publish::<be_sdk_api::DisplaysWatch>(&displays);
}

fn host_display(monitor: &Monitor, shown: Option<&Display>) -> HostDisplay {
    HostDisplay {
        id: monitor.id.clone(),
        name: monitor.name.clone(),
        connector: monitor.connector.clone(),
        modes: monitor.modes.clone(),
        default: monitor.default,
        current: monitor.current,
        rotation: monitor.rotation,
        rect: shown.map_or(beui::Rect::ZERO, Display::rect),
        scale: shown.map_or(monitor.default_scale, |display| display.scale),
        default_scale: monitor.default_scale,
    }
}

#[cfg(test)]
mod tests;
