use be_lib_geometry::{Rect, pos2};
use be_lib_ui_types::input::Keystroke;

use super::*;

/// Surfaces as rects in z-order (bottom first), each placed by another, granting its placer
/// gestures over `gestures`.
#[derive(Default)]
struct Rects {
    surfaces: Vec<Surface>,
    /// Where a press pans the camera rather than pressing anything.
    pans: Option<Rect>,
}

struct Surface {
    id: u32,
    rect: Rect,
    placer: Option<u32>,
    gestures: Option<Rect>,
    focus: bool,
    captures: bool,
}

impl Rects {
    fn with(mut self, id: u32, rect: Rect, placer: Option<u32>) -> Self {
        self.surfaces.push(Surface {
            id,
            rect,
            placer,
            gestures: None,
            focus: true,
            captures: false,
        });
        self
    }

    fn get(&self, id: u32) -> Option<&Surface> {
        self.surfaces.iter().find(|surface| surface.id == id)
    }

    fn last(&mut self) -> &mut Surface {
        self.surfaces.last_mut().expect("a surface")
    }
}

impl HitTest for Rects {
    type Target = u32;

    fn hit(&self, at: Pos2) -> Option<u32> {
        for surface in self.surfaces.iter().rev() {
            if surface.rect.contains(at) {
                return Some(surface.id);
            }
            if surface.captures {
                return surface.placer;
            }
        }
        None
    }

    fn local(&self, target: u32, at: Pos2) -> Option<Pos2> {
        let rect = self.get(target)?.rect;
        Some(pos2(at.x - rect.min.x, at.y - rect.min.y))
    }

    fn placer(&self, target: u32) -> Option<u32> {
        self.get(target)?.placer
    }

    fn grants_gestures(&self, target: u32, at: Pos2) -> bool {
        self.get(target)
            .and_then(|surface| surface.gestures)
            .is_some_and(|area| area.contains(at))
    }

    fn takes_focus(&self, target: u32) -> bool {
        self.get(target).is_some_and(|surface| surface.focus)
    }

    fn pans(&self, at: Pos2) -> bool {
        self.pans.is_some_and(|area| area.contains(at))
    }
}

const ROOT: u32 = 0;
const CANVAS: u32 = 1;
const EDITOR: u32 = 2;

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_max(pos2(x, y), pos2(x + width, y + height))
}

/// The compositor's surface under a canvas it places, with an editor the canvas places on it.
fn canvas_with_editor() -> Rects {
    Rects::default()
        .with(ROOT, rect(0.0, 0.0, 1000.0, 1000.0), None)
        .with(CANVAS, rect(100.0, 100.0, 600.0, 600.0), Some(ROOT))
        .with(EDITOR, rect(200.0, 200.0, 200.0, 200.0), Some(CANVAS))
}

fn press(x: f32, y: f32, pressed: bool) -> Input {
    button(PointerButton::Primary, x, y, pressed)
}

fn button(button: PointerButton, x: f32, y: f32, pressed: bool) -> Input {
    Input::PointerButton {
        pos: pos2(x, y),
        button,
        pressed,
    }
}

fn touch(finger: u64, phase: TouchPhase, x: f32, y: f32) -> Input {
    Input::Touch {
        id: TouchId { device: 0, finger },
        phase,
        pos: pos2(x, y),
        force: None,
    }
}

fn key(key: Key, pressed: bool) -> Input {
    Input::Key(Keystroke {
        key: Some(key),
        pressed,
        repeat: false,
        evdev: None,
        text: None,
    })
}

fn targets(routed: &[Routed<u32>]) -> Vec<u32> {
    routed
        .iter()
        .map(|routed| match routed {
            Routed::Input { target, .. } | Routed::Drag { target, .. } => *target,
        })
        .collect()
}

fn inputs_for(routed: &[Routed<u32>], to: u32) -> Vec<Input> {
    routed
        .iter()
        .filter_map(|routed| match routed {
            Routed::Input { target, input } if *target == to => Some(input.clone()),
            _ => None,
        })
        .collect()
}

mod a_desktop_windows_second_button_goes_to_it_while_the_first_holds_it;
mod a_middle_press_that_pans_goes_to_no_surface_even_one_the_mouse_holds;
mod a_programs_pen_and_the_mouse_each_hold_the_surface_they_pressed;
mod an_anchored_view_takes_presses_on_it_and_its_apps_view_those_beside_it;
mod an_apps_view_hears_the_pointer_leave_once_no_press_holds_it;
mod the_on_screen_keyboard_takes_presses_while_keys_stay_with_the_focused_field;
mod a_window_that_never_takes_the_focus_leaves_it_with_the_window_that_had_it;
mod a_scroll_begun_where_a_camera_claims_it_stays_with_the_camera;
mod a_held_gesture_that_never_ends_lets_go_once_the_pointer_moves_or_leaves;
mod a_gesture_latched_to_a_surface_that_went_goes_where_the_pointer_is;
mod a_drag_goes_over_the_surfaces_under_the_pointer_and_drops_on_the_one_under_the_release;
mod a_finger_landing_for_a_pinch_its_placer_claims_takes_the_earlier_fingers_with_it;
mod a_forgotten_surface_holds_nothing_and_hears_nothing;
mod a_held_press_handed_to_the_placer_ends_cancelled_in_the_editor;
mod a_keys_release_goes_to_the_focus_which_the_surface_that_lost_it_heard_of_first;
mod a_pen_down_holds_its_surface_until_it_lifts;
mod a_press_captures_the_pointer_for_its_surface_until_every_button_lifts;
mod a_press_focuses_its_surface_unless_it_never_takes_the_focus;
mod a_press_outside_a_surface_that_captures_presses_goes_to_its_placer;
mod a_scroll_and_a_touchpad_gesture_stay_with_the_surface_they_began_on;
mod a_wheel_over_an_editor_goes_to_the_outermost_camera_it_grants_the_gesture;
mod cancelled_pointers_end_the_press_the_pen_the_fingers_and_a_carry;
mod hover_follows_the_surface_under_the_pointer;
mod keys_go_to_the_focus_and_the_compositors_chords_go_to_it_first;
mod only_the_focused_surface_holds_the_pointer_lock_and_loses_it_with_the_focus;
mod two_fingers_on_one_editor_stay_with_it;
