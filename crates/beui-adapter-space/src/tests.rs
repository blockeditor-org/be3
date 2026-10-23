use std::time::{Duration, Instant};

use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::input::{Input, Key, Keystroke, PointerButton, ScrollDelta};
use beui_core::runner::{HostView, Incoming, Units, WindowHost};
use beui_core::window::{WindowId, WindowReport};
use glam::Vec3;

use crate::Space;
use crate::head::Head;
use crate::panel::{Panel, Pose};

mod a_carried_panel_follows_the_cursor_and_the_head;
mod a_click_on_the_panel_reaches_the_app_where_the_ray_hits;
mod a_miss_leaves_the_panel;
mod a_ray_hits_the_nearest_panel_where_it_points;
mod a_released_panel_turns_to_face_the_viewer;
mod holding_right_ctrl_and_right_shift_steers_the_head;
mod scrolling_while_carrying_moves_the_panel_nearer_or_farther;

const PLATFORM: Vec2 = vec2(800.0, 600.0);

fn view(now: Instant) -> HostView {
    HostView {
        platform: PLATFORM,
        pixels_per_point: 1.0,
        now,
    }
}

fn space() -> (Space, Instant) {
    let now = Instant::now();
    let mut space = Space::new();
    let mut incoming = Vec::new();
    space.input(&mut incoming, &view(now));
    (space, now)
}

fn send(space: &mut Space, now: Instant, inputs: impl IntoIterator<Item = Input>) -> Vec<Incoming> {
    let mut incoming: Vec<Incoming> = inputs
        .into_iter()
        .map(|input| Incoming::new(Units::Physical, input))
        .collect();
    space.input(&mut incoming, &view(now));
    incoming
}

fn key(key: Key, evdev: u32, pressed: bool) -> Input {
    Input::Key(Keystroke {
        key: Some(key),
        pressed,
        evdev: Some(evdev),
        ..Keystroke::default()
    })
}

const RIGHT_CTRL: u32 = 97;
const RIGHT_SHIFT: u32 = 54;

fn steer(pressed: bool) -> [Input; 2] {
    [
        key(Key::Ctrl, RIGHT_CTRL, pressed),
        key(Key::Shift, RIGHT_SHIFT, pressed),
    ]
}

fn button(pos: Pos2, pressed: bool) -> Input {
    Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
    }
}

fn centre() -> Pos2 {
    pos2(PLATFORM.x / 2.0, PLATFORM.y / 2.0)
}

fn delivered(incoming: &[Incoming]) -> Vec<(WindowId, Input)> {
    incoming
        .iter()
        .filter_map(|incoming| match incoming {
            Incoming::Input { window, input, .. } => Some((*window, input.clone())),
            _ => None,
        })
        .collect()
}

fn main(space: &Space) -> &Panel {
    space
        .panel(WindowId::MAIN)
        .expect("the main window has a panel")
}

fn close(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < 1e-3
}

fn ahead(distance: f32) -> Pose {
    Pose {
        position: Head::default().position + Vec3::new(0.0, 0.0, -distance),
        rotation: glam::Quat::IDENTITY,
    }
}

fn panel(window: WindowId, pose: Pose) -> Panel {
    Panel {
        window,
        pose,
        size: vec2(1000.0, 500.0),
    }
}

fn later(now: Instant, seconds: f32) -> Instant {
    now + Duration::from_secs_f32(seconds)
}

fn placed(incoming: &[Incoming]) -> Vec<WindowReport> {
    incoming
        .iter()
        .filter_map(|incoming| match incoming {
            Incoming::Window(report @ WindowReport::Placed { .. }) => Some(report.clone()),
            _ => None,
        })
        .collect()
}

fn scroll(lines: f32) -> Input {
    Input::Scroll(ScrollDelta::Lines(vec2(0.0, lines)))
}
