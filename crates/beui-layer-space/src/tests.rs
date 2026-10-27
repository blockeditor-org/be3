use std::time::{Duration, Instant};

use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::Rect;
use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::input::{
    CursorIcon, Input, Key, Keystroke, Modifiers, PointerButton, ScrollDelta, TouchPhase,
};
use beui_core::runner::{Controller, Hand, HandPose, Incoming, LayerView, Units};
use beui_core::window::{
    HostPlacement, WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest, WindowRole,
};
use glam::Vec3;

use crate::chrome::{self, Corner, Part};
use crate::head::Head;
use crate::panel::{Panel, Pose};
use crate::{Pointer, Space};

mod a_bracket_in_front_of_another_panel_takes_the_click;
mod a_carried_panel_follows_the_cursor_and_the_head;
mod a_carried_panel_let_go_mid_turn_lands_where_it_would_after_the_turn;
mod a_carried_panel_swings_about_the_view_s_up_with_the_head_rolled;
mod a_carried_panel_turns_smoothly_to_face_the_viewer_while_carried;
mod a_click_on_the_panel_reaches_the_app_where_the_ray_hits;
mod a_cursor_turned_out_of_view_shows_a_mouse_at_the_edge_pointing_at_it;
mod a_driver_pointing_at_a_window_shows_its_icon;
mod a_laser_drags_once_it_turns_past_where_the_trigger_froze_it;
mod a_laser_is_smoothed_and_takes_a_jump_at_once;
mod a_layered_panel_is_left_clear_in_the_eyes_and_copied_into_its_layer;
mod a_miss_keeps_the_cursor_as_far_away_as_it_last_hit;
mod a_miss_leaves_the_panel;
mod a_miss_while_a_button_is_held_leaves_the_window_and_keeps_the_drag;
mod a_move_asked_for_after_its_click_was_let_go_carries_nothing;
mod a_nearer_panel_is_drawn_at_more_pixels_per_point;
mod a_new_pixels_per_point_waits_for_the_panel_to_settle_and_not_while_it_is_resized;
mod a_panel_carried_above_eye_level_tilts_to_face_the_eye;
mod a_panel_carried_by_its_grip;
mod a_panel_carried_to_each_corner_with_the_head_rolled_and_pitched_stays_upright_to_the_view;
mod a_panel_carried_under_the_floor_stays_visible_and_can_be_picked;
mod a_panel_carried_with_the_head_rolled_rolls_to_match;
mod a_panel_is_rastered_at_the_most_any_view_needs;
mod a_panel_keeps_its_pixels_per_point_until_its_size_on_screen_changes_a_step;
mod a_pulled_trigger_touches_the_window_where_its_laser_hits;
mod a_ray_between_a_window_and_its_chrome_never_falls_through_to_a_window_behind;
mod a_ray_hits_a_curved_panel_on_its_bent_surface;
mod a_ray_hits_the_nearest_panel_where_it_points;
mod a_released_panel_turns_to_face_the_viewer;
mod a_resized_panel_keeps_its_renderer_and_draws_at_the_new_size;
mod a_scrolled_panel_copies_what_it_showed_and_paints_what_a_full_repaint_would;
mod a_wheel_line_moves_a_carried_panel_a_step_and_pixels_in_proportion;
mod a_window_asked_to_resize_keeps_its_top_left_corner_and_reports_the_size;
mod a_window_carried_from_a_press_in_another_gives_that_one_its_release;
mod a_window_carried_over_the_main_panel_keeps_its_distance;
mod a_window_moved_by_a_laser_touch_is_carried_along_that_laser;
mod a_window_opened_at_a_host_placement_lands_on_that_pose;
mod a_window_opened_at_the_pointer_lands_on_the_cursor_ray_facing_the_viewer;
mod a_window_opened_over_another_sits_where_that_window_would_show_it_a_little_in_front;
mod a_window_opened_over_the_main_panel_takes_the_focus_and_the_keys;
mod a_window_point_maps_to_the_screen_and_back_through_its_panel;
mod a_window_s_notice_shows_in_place_of_its_title;
mod an_absolute_position_puts_the_cursor_where_its_ray_hits;
mod an_off_axis_view_projects_its_field_of_view_tangents_to_the_edges_of_its_target;
mod clicking_a_panel_or_its_chrome_focuses_it_until_another_or_nothing_is_clicked;
mod closing_a_window_removes_its_panel;
mod dragging_a_corner_handle_scales_the_panel_and_keeps_its_points;
mod dragging_a_side_of_the_main_panel_resizes_its_window_and_keeps_its_scale;
mod dragging_the_move_control_carries_the_panel;
mod dragging_the_roll_control_rolls_the_panel_and_the_rotate_control_never_does;
mod dragging_the_roll_control_with_the_head_rolled_rolls_the_panel_about_its_normal;
mod dragging_the_rotate_control_keeps_the_cursor_on_the_button;
mod dragging_the_rotate_control_turns_the_panel_as_seen_from_the_eye_wherever_it_is_on_screen;
mod dragging_the_rotate_control_turns_the_panel_until_it_is_carried;
mod each_panel_shows_its_window_s_title_and_dragging_the_title_carries_it;
mod each_tracked_view_is_drawn_into_its_own_target_through_its_own_field_of_view;
mod holding_ctrl_and_shift_steers_the_head;
mod holding_escape_lets_the_pointer_go_until_the_next_click;
mod input_reaches_the_window_whose_panel_the_ray_hits_in_its_points;
mod keys_a_driver_sends_steer_too;
mod losing_focus_lets_the_pointer_go_and_the_click_back_is_swallowed;
mod motion_a_driver_sends_turns_the_head_while_steering;
mod mouse_motion_up_moves_the_cursor_toward_the_sky_however_the_head_is_tilted;
mod moving_a_window_carries_its_panel;
mod panels_get_layers_only_where_their_order_to_the_eye_is_plain;
mod panels_through_each_other_are_painted_nearest_first_at_every_pixel;
mod q_and_e_roll_the_head_while_steering;
mod resizing_a_window_drags_its_edge_and_reports_the_new_size;
mod scrolling_while_carrying_moves_the_panel_nearer_or_farther;
mod squeezing_a_panel_carries_it_along_the_laser;
mod squeezing_a_panel_with_both_hands_scales_and_turns_it;
mod steering_with_a_headset_moves_the_body_the_views_are_tracked_in;
mod switching_the_cursor_icon_keeps_the_cursor_renderer;
mod tapping_escape_reaches_the_app_and_keeps_the_grab;
mod the_chrome_around_a_curved_panel;
mod the_chrome_keeps_its_size_to_the_eye_at_any_distance;
mod the_chrome_of_a_near_panel_draws_its_icons_at_the_panel_s_raster;
mod the_cursor_is_drawn_only_while_the_pointer_is_grabbed;
mod the_cursor_shows_the_icon_the_window_under_it_asks_for_and_hides_the_platform_one;
mod the_cursor_sits_just_in_front_of_the_panel_at_the_same_size_to_the_eye;
mod the_cursor_sprite_draws_the_icon_around_its_hotspot;
mod the_curve_slider_bends_the_panel_and_clicks_land_on_the_curve;
mod the_keyboard_opens_under_the_focused_field_and_never_takes_the_focus;
mod the_lasers_are_drawn_to_where_they_hit;
mod the_mirror_shows_a_notice_over_the_scene;
mod the_mouse_moves_the_cursor_past_the_edge_of_the_view;
mod the_platform_dropping_the_lock_lets_the_pointer_go;
mod tracked_views_move_the_head_to_their_centre_and_set_the_rasters;
mod turning_away_and_back_leaves_the_cursor_where_it_was;
mod two_lasers_on_one_panel_are_two_fingers;
mod two_lasers_pinching_a_panel;
mod walking_keeps_the_cursor_at_its_world_point;
mod windows_opened_over_the_same_spot_stack_toward_the_viewer;

const PLATFORM: Vec2 = vec2(800.0, 600.0);

fn view(now: Instant) -> LayerView {
    LayerView {
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
        role: WindowRole::Normal,
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
            Incoming::Window(WindowReport::Rastered { .. } | WindowReport::Focused { .. }) => None,
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
            Incoming::Window(WindowReport::Rastered {
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
    let layout = chrome::layout(vec2(metres.x, metres.y), panel.chrome, slider);
    let (_, rect) = layout
        .buttons
        .into_iter()
        .chain([(Part::Slider, layout.track)])
        .chain(layout.parts)
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

fn drive(
    space: &mut Space,
    now: Instant,
    controllers: impl IntoIterator<Item = Controller>,
) -> Vec<Incoming> {
    let mut incoming: Vec<Incoming> = controllers.into_iter().map(Incoming::Controller).collect();
    space.input(&mut incoming, &view(now));
    incoming
}

fn hand_pose(origin: Vec3, target: Vec3) -> Controller {
    hand_pose_of(Hand::Right, origin, target)
}

fn hand_pose_of(hand: Hand, origin: Vec3, target: Vec3) -> Controller {
    let pose = HandPose {
        position: origin.to_array(),
        orientation: crate::laser::looking(target - origin).to_array(),
    };
    Controller::Pose {
        hand,
        aim: pose,
        grip: pose,
    }
}

fn trigger(hand: Hand, value: f32) -> Controller {
    Controller::Trigger { hand, value }
}

fn squeeze(hand: Hand, value: f32) -> Controller {
    Controller::Squeeze { hand, value }
}

fn left_hand() -> Vec3 {
    Head::default().position + Vec3::new(-0.2, -0.35, -0.3)
}

fn right_hand() -> Vec3 {
    Head::default().position + Vec3::new(0.2, -0.35, -0.3)
}

fn touches(incoming: &[Incoming]) -> Vec<(WindowId, u64, TouchPhase, Pos2)> {
    delivered(incoming)
        .into_iter()
        .filter_map(|(window, input)| match input {
            Input::Touch { id, phase, pos, .. } => {
                assert_eq!(id.device, crate::laser::DEVICE);
                Some((window, id.finger, phase, pos))
            }
            _ => None,
        })
        .collect()
}

fn near(a: Pos2, b: Pos2) -> bool {
    (a - b).length() < 0.5
}

fn off_ray(point: Vec3, origin: Vec3, toward: Vec3) -> f32 {
    let direction = (toward - origin).normalize();
    (point - origin).cross(direction).length()
}

fn painted_space() -> (Space, Instant, Context) {
    let (mut space, now) = space();
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let mut setup = beui_core::app::Setup::new(beui_core::app::Waker::new(|| {}));
    setup.provide(context.clone());
    space.setup(&setup);
    (space, now, context)
}

fn assert_scene(
    space: &mut Space,
    now: Instant,
    windows: &[beui_core::context::WindowOutput],
    name: &str,
) {
    let frame = space.scene(windows, &view(now));
    let painting = crate::picture::painting(&frame, Rect::from_min_size(Pos2::ZERO, PLATFORM))
        .expect("the scene paints");
    let snapshot = paint_snapshot::Snapshot::of_painting(painting).expect("the painting encodes");
    paint_snapshot::assert_accepted(name, &snapshot, &format!("./scripts/test {name} --update"));
}
