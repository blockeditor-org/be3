use std::time::{Duration, Instant};

use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::Rect;
use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::input::{CursorIcon, Input, Key, Keystroke, Modifiers, PointerButton, ScrollDelta};
use beui_core::runner::{HostView, Incoming, Units, WindowHost};
use beui_core::window::{
    HostPlacement, WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest,
};
use glam::Vec3;

use crate::Space;
use crate::chrome::{self, Corner, Part};
use crate::head::Head;
use crate::panel::{Panel, Pose};

mod a_bracket_in_front_of_another_panel_takes_the_click;
mod a_carried_panel_follows_the_cursor_and_the_head;
mod a_carried_panel_moved_off_what_it_is_over_returns_smoothly_to_its_distance;
mod a_carried_panel_stays_in_front_of_a_curved_panel;
mod a_carried_panel_stays_in_front_of_the_panel_it_is_carried_over_and_after_release;
mod a_carried_panel_turns_smoothly_to_face_the_viewer_while_carried;
mod a_click_on_the_panel_reaches_the_app_where_the_ray_hits;
mod a_cursor_turned_out_of_view_is_drawn_inside_the_edge_and_clicks_there;
mod a_driver_pointing_at_a_window_shows_its_icon;
mod a_miss_keeps_the_cursor_as_far_away_as_it_last_hit;
mod a_miss_leaves_the_panel;
mod a_miss_while_a_button_is_held_leaves_the_window_and_keeps_the_drag;
mod a_nearer_panel_is_drawn_at_more_pixels_per_point;
mod a_new_pixels_per_point_waits_for_the_panel_to_settle_and_not_while_it_is_resized;
mod a_panel_carried_above_eye_level_tilts_to_face_the_eye;
mod a_panel_carried_under_the_floor_stays_visible_and_can_be_picked;
mod a_panel_carried_to_each_corner_with_the_head_rolled_and_pitched_stays_upright_to_the_view;
mod a_panel_carried_with_the_head_rolled_rolls_to_match;
mod a_panel_keeps_its_pixels_per_point_until_its_size_on_screen_changes_a_step;
mod a_ray_hits_a_curved_panel_on_its_bent_surface;
mod a_ray_hits_the_nearest_panel_where_it_points;
mod a_released_panel_turns_to_face_the_viewer;
mod a_resized_panel_keeps_its_renderer_and_draws_at_the_new_size;
mod a_scrolled_panel_copies_what_it_showed_and_paints_what_a_full_repaint_would;
mod a_window_asked_to_resize_keeps_its_top_left_corner_and_reports_the_size;
mod a_window_carried_over_the_main_panel_and_let_go_is_drawn_in_front;
mod a_window_opened_at_a_host_placement_lands_on_that_pose;
mod a_window_opened_at_the_pointer_lands_on_the_cursor_ray_facing_the_viewer;
mod a_window_opened_over_another_sits_where_that_window_would_show_it_a_little_in_front;
mod a_window_opened_over_the_main_panel_takes_the_focus_and_the_keys;
mod an_absolute_position_puts_the_cursor_where_its_ray_hits;
mod clicking_a_panel_or_its_chrome_focuses_it_until_another_or_nothing_is_clicked;
mod closing_a_window_removes_its_panel;
mod dragging_a_corner_handle_scales_the_panel_and_keeps_its_points;
mod dragging_the_move_control_carries_the_panel;
mod dragging_the_roll_control_rolls_the_panel_and_the_rotate_control_never_does;
mod dragging_the_roll_control_with_the_head_rolled_rolls_the_panel_about_its_normal;
mod dragging_the_rotate_control_turns_the_panel_until_it_is_carried;
mod dragging_the_rotate_control_with_the_head_rolled_turns_the_panel_along_the_view;
mod holding_ctrl_and_shift_steers_the_head;
mod holding_escape_lets_the_pointer_go_until_the_next_click;
mod input_reaches_the_window_whose_panel_the_ray_hits_in_its_points;
mod keys_a_driver_sends_steer_too;
mod losing_focus_lets_the_pointer_go_and_the_click_back_is_swallowed;
mod motion_a_driver_sends_turns_the_head_while_steering;
mod moving_a_window_carries_its_panel;
mod moving_the_mouse_from_the_edge_starts_where_the_cursor_is_drawn;
mod q_and_e_roll_the_head_while_steering;
mod resizing_a_window_drags_its_edge_and_reports_the_new_size;
mod scrolling_a_carried_panel_farther_than_the_one_behind_puts_it_behind;
mod scrolling_while_carrying_moves_the_panel_nearer_or_farther;
mod switching_the_cursor_icon_keeps_the_cursor_renderer;
mod tapping_escape_reaches_the_app_and_keeps_the_grab;
mod the_chrome_around_a_curved_panel;
mod the_cursor_moves_along_the_screen_with_the_head_rolled;
mod the_cursor_shows_the_icon_the_window_under_it_asks_for_and_hides_the_platform_one;
mod the_cursor_sits_just_in_front_of_the_panel_at_the_same_size_to_the_eye;
mod the_cursor_sprite_draws_the_icon_around_its_hotspot;
mod the_curve_slider_bends_the_panel_and_clicks_land_on_the_curve;
mod the_platform_dropping_the_lock_lets_the_pointer_go;
mod turning_away_and_back_leaves_the_cursor_where_it_was;
mod walking_keeps_the_cursor_at_its_world_point;
mod windows_opened_over_the_same_spot_stack_toward_the_viewer;

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
    Panel::new(window, pose, vec2(1000.0, 500.0))
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
        WindowPlacement::Host(panel(WindowId::MAIN, ahead(distance)).placement()),
    )
    .0
}

fn reports(incoming: &[Incoming]) -> Vec<WindowReport> {
    incoming
        .iter()
        .filter_map(|incoming| match incoming {
            Incoming::Window(WindowReport::Scaled { .. } | WindowReport::Focused { .. }) => None,
            Incoming::Window(report) => Some(report.clone()),
            _ => None,
        })
        .collect()
}

fn focus_reports(incoming: &[Incoming]) -> Vec<(WindowId, bool)> {
    incoming
        .iter()
        .filter_map(|incoming| match incoming {
            Incoming::Window(WindowReport::Focused { window, focused }) => {
                Some((*window, *focused))
            }
            _ => None,
        })
        .collect()
}

fn keys(incoming: &[Incoming]) -> usize {
    delivered(incoming)
        .iter()
        .filter(|(_, input)| matches!(input, Input::Key(_)))
        .count()
}

fn scaled(incoming: &[Incoming]) -> Vec<(WindowId, f32)> {
    incoming
        .iter()
        .filter_map(|incoming| match incoming {
            Incoming::Window(WindowReport::Scaled {
                window,
                pixels_per_point,
            }) => Some((*window, *pixels_per_point)),
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

fn app_frame(cursor: CursorIcon) -> FrameOutput {
    let fonts = beui_font_freetype::FreetypeFonts::new(beui_font_freetype::FontLibrary::new(
        beui_font_freetype::FontSources::default(),
    ));
    Context::new(fonts).run(|context| context.set_cursor_icon(cursor))
}

fn eye_distance(space: &Space) -> f32 {
    space
        .cursor_at()
        .expect("the cursor is shown")
        .distance(space.head.position)
}

fn screen(space: &Space, world: Vec3) -> Pos2 {
    let clip = space.head.view_projection(PLATFORM) * world.extend(1.0);
    pos2(
        (clip.x / clip.w + 1.0) / 2.0 * PLATFORM.x,
        (1.0 - clip.y / clip.w) / 2.0 * PLATFORM.y,
    )
}

fn part_pos(panel: &Panel, part: Part, slider: bool) -> Pos2 {
    let metres = panel.metres();
    let (_, rect) = chrome::layout(vec2(metres.x, metres.y), slider)
        .parts
        .into_iter()
        .find(|(found, _)| *found == part)
        .expect("the chrome has that part");
    let centre = rect.center();
    pos2(
        centre.x / panel.metres_per_point,
        centre.y / panel.metres_per_point,
    )
}

fn on_part(space: &Space, window: WindowId, part: Part) -> Pos2 {
    let panel = space.panel(window).expect("the window has a panel");
    let slider = space.chrome.slider == Some(window);
    screen(space, panel.world(part_pos(panel, part, slider)))
}

fn facing_eye(space: &Space, panel: &Panel) -> f32 {
    panel
        .normal()
        .angle_between(space.head.position - panel.pose.position)
}

fn roll_and_look_down(space: &mut Space) {
    space.head.roll = std::f32::consts::FRAC_PI_2;
    space.head.pitch = -0.5;
    let head = space.head;
    let panel = &mut space.panels[0];
    panel.pose.position = head.position + head.rotation() * Vec3::new(0.0, 0.0, -1.5);
    panel.face(head.position, head.up());
}

fn upright_to(space: &Space, panel: &Panel) -> f32 {
    let normal = panel.normal();
    let up = space.head.up();
    (panel.pose.rotation * Vec3::Y).angle_between(up - normal * up.dot(normal))
}

fn level(panel: &Panel) -> f32 {
    (panel.pose.rotation * Vec3::X).y.abs()
}

fn carry_popped_out(space: &mut Space, now: Instant) -> WindowId {
    let (window, _) = open(
        space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(20.0, 20.0),
        },
    );
    let middle = pos2(SMALL.x / 2.0, SMALL.y / 2.0);
    let grab = screen(space, space.panel(window).unwrap().world(middle));
    send(space, now, [Input::PointerMoved(grab), button(grab, true)]);
    space.request(&WindowRequest::BeginMove(window));
    send(space, now, []);
    window
}

fn gaps_to(space: &Space, window: WindowId, behind: WindowId) -> Vec<f32> {
    let (carried, other) = (space.panel(window).unwrap(), space.panel(behind).unwrap());
    let eye = space.head.position;
    [0.0, 0.5, 1.0]
        .into_iter()
        .flat_map(|x| [0.0, 0.5, 1.0].map(|y| pos2(x * carried.size.x, y * carried.size.y)))
        .filter_map(|pos| {
            let point = carried.world(pos);
            let ray = crate::head::Ray {
                origin: eye,
                direction: (point - eye).normalize(),
            };
            let hit = other.hit(ray)?;
            Some(hit.distance - point.distance(eye))
        })
        .collect()
}
