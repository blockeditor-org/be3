use std::time::{Duration, Instant};

use beui_core::geometry::Rect;
use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::input::{Input, Key, Keystroke, Modifiers, PointerButton, ScrollDelta};
use beui_core::runner::{HostView, Incoming, Units, WindowHost};
use beui_core::window::{
    HostPlacement, WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest,
};
use glam::Vec3;

use crate::Space;
use crate::head::Head;
use crate::panel::{Panel, Pose};

mod a_carried_panel_follows_the_cursor_and_the_head;
mod a_carried_panel_is_in_front_of_the_others_until_scrolled_away;
mod a_carried_panel_turns_smoothly_to_face_the_viewer_while_carried;
mod a_click_on_the_panel_reaches_the_app_where_the_ray_hits;
mod a_miss_leaves_the_panel;
mod a_miss_while_a_button_is_held_leaves_the_window_and_keeps_the_drag;
mod a_ray_hits_the_nearest_panel_where_it_points;
mod a_released_panel_turns_to_face_the_viewer;
mod a_window_asked_to_resize_keeps_its_top_left_corner_and_reports_the_size;
mod a_window_opened_at_a_host_placement_lands_on_that_pose;
mod a_window_opened_at_the_pointer_lands_on_the_cursor_ray_facing_the_viewer;
mod a_window_opened_beside_another_sits_to_its_right;
mod closing_a_window_removes_its_panel;
mod holding_ctrl_and_shift_steers_the_head;
mod input_reaches_the_window_whose_panel_the_ray_hits_in_its_points;
mod keys_a_driver_sends_steer_too;
mod motion_a_driver_sends_turns_the_head_while_steering;
mod moving_a_window_carries_its_panel;
mod resizing_a_window_drags_its_edge_and_reports_the_new_size;
mod scrolling_while_carrying_moves_the_panel_nearer_or_farther;
mod windows_opened_beside_the_same_one_line_up_to_its_right;

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

const CTRL: u32 = 29;
const SHIFT: u32 = 42;

fn steer(pressed: bool) -> [Input; 4] {
    let ctrl = Modifiers::CTRL;
    let chord = Modifiers {
        shift: true,
        ..Modifiers::CTRL
    };
    match pressed {
        true => [
            key(Key::Ctrl, CTRL, true),
            Input::Modifiers(ctrl),
            key(Key::Shift, SHIFT, true),
            Input::Modifiers(chord),
        ],
        false => [
            key(Key::Shift, SHIFT, false),
            Input::Modifiers(ctrl),
            key(Key::Ctrl, CTRL, false),
            Input::Modifiers(Modifiers::NONE),
        ],
    }
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

const SMALL: Vec2 = vec2(200.0, 150.0);

fn open(space: &mut Space, now: Instant, placement: WindowPlacement) -> (WindowId, Vec<Incoming>) {
    let window = WindowId::next();
    space.request(&WindowRequest::Open {
        window,
        placement,
        size: SMALL,
    });
    (window, send(space, now, []))
}

fn open_ahead(space: &mut Space, now: Instant, distance: f32) -> WindowId {
    open(
        space,
        now,
        WindowPlacement::Host(ahead(distance).placement()),
    )
    .0
}

fn reports(incoming: &[Incoming]) -> Vec<WindowReport> {
    incoming
        .iter()
        .filter_map(|incoming| match incoming {
            Incoming::Window(report) => Some(report.clone()),
            _ => None,
        })
        .collect()
}

fn pos_of(input: &Input) -> Pos2 {
    match input {
        Input::PointerMoved(pos) | Input::PointerButton { pos, .. } => *pos,
        other => panic!("no position in {other:?}"),
    }
}

fn on(space: &Space, window: WindowId, at: Pos2) -> Pos2 {
    space
        .panel(window)
        .and_then(|panel| panel.hit(space.head.ray(PLATFORM, at.to_vec2())))
        .expect("the cursor is on the panel")
        .pos
}
