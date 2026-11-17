use be_lib_geometry::{Rect, pos2};
use be_lib_ui_types::input::Keystroke;

use super::*;

/// Surfaces as rects in z-order (bottom first), each placed by another, granting its placer
/// gestures over `gestures`.
#[derive(Default)]
struct Rects {
    surfaces: Vec<Surface>,
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
    Input::PointerButton {
        pos: pos2(x, y),
        button: PointerButton::Primary,
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

mod a_drag_goes_over_the_surfaces_under_the_pointer_and_drops_on_the_one_under_the_release;
mod a_finger_landing_for_a_pinch_its_placer_claims_takes_the_earlier_fingers_with_it;
mod a_forgotten_surface_holds_nothing_and_hears_nothing;
mod a_held_press_handed_to_the_placer_ends_cancelled_in_the_editor;
mod a_keys_release_follows_its_press_after_the_focus_moves;
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
