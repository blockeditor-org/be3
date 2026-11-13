use std::cell::RefCell;

use be_block::DisplaySettings;
use be_block::display_settings::ScreenUse;
use be_sdk_api::{HostDisplay, ScreenEdge, ScreenEdgeState, ScreenSide};
use beui_platform_drm::{
    Display, DisplayConfig, DisplayControl, DisplayUse, Displays, EdgeState, Epaper, Monitor,
    ScreenEdges, Side,
};

use super::Screen;
use super::shown::ShownDisplay;

thread_local! {
    static CONTROL: RefCell<Option<DisplayControl>> = const { RefCell::new(None) };
    static EPAPER: RefCell<Option<Epaper>> = const { RefCell::new(None) };
    static MONITORS: RefCell<Vec<Monitor>> = const { RefCell::new(Vec::new()) };
    static DISPLAYS: RefCell<Vec<Display>> = const { RefCell::new(Vec::new()) };
    static EDGES: RefCell<Option<ScreenEdges>> = const { RefCell::new(None) };
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
    let edges = setup.get::<ScreenEdges>().cloned();
    if let Some(edges) = &edges {
        let watched = edges.clone();
        let publish = move || {
            let states: Vec<ScreenEdgeState> =
                watched.states().into_iter().map(edge_state).collect();
            be_host_base::watches::publish::<be_sdk_api::ScreenEdgesWatch>(&states);
            be_host_base::host::wake();
        };
        publish();
        edges.watch(publish);
    }
    EDGES.with(|slot| *slot.borrow_mut() = edges);
}

pub(crate) fn epaper() -> bool {
    EPAPER.with(|slot| slot.borrow().is_some())
}

pub(crate) fn stop() {
    let control = CONTROL.with(|slot| slot.borrow_mut().take());
    drop(control);
    let epaper = EPAPER.with(|slot| slot.borrow_mut().take());
    drop(epaper);
    let edges = EDGES.with(|slot| slot.borrow_mut().take());
    drop(edges);
}

/// Hands the shell's screen edges to the platform, which walls the pointer at them.
pub(crate) fn run_edges() {
    let Some(edges) = be_host_base::watches::take_calls(be_sdk_api::requested::set_edges).pop()
    else {
        return;
    };
    EDGES.with(|slot| {
        if let Some(held) = slot.borrow().as_ref() {
            held.set(edges.into_iter().map(screen_edge).collect());
        }
    });
}

fn screen_edge(edge: ScreenEdge) -> beui_platform_drm::ScreenEdge {
    beui_platform_drm::ScreenEdge {
        id: edge.id,
        display: edge.display,
        side: match edge.side {
            ScreenSide::Top => Side::Top,
            ScreenSide::Bottom => Side::Bottom,
            ScreenSide::Left => Side::Left,
            ScreenSide::Right => Side::Right,
        },
        start: edge.start,
        end: edge.end,
        wall: edge.wall,
        deliberate: edge.deliberate,
        keep: edge.keep,
    }
}

fn edge_state(state: EdgeState) -> ScreenEdgeState {
    ScreenEdgeState {
        id: state.id,
        pushed: state.pushed,
        near: state.near,
        pull: state.pull,
        pulls: state.pulls,
        pulled: state.pulled,
        taps_away: state.taps_away,
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
    be_host_desktop::desktop_host::configure(|desktop| {
        desktop.blank_after = settings.screen_off().after();
        desktop.lock_after = settings.lock_time();
    });
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
