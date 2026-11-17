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

#[test]
fn a_press_captures_the_pointer_for_its_surface_until_every_button_lifts() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    assert_eq!(router.pressed(), Some(EDITOR));
    let moved = router.route(&test, Input::PointerMoved(pos2(900.0, 900.0)));
    assert_eq!(
        moved,
        vec![Routed::Input {
            target: EDITOR,
            input: Input::PointerMoved(pos2(700.0, 700.0)),
        }],
        "a held press follows the pointer out of its surface, in the surface's coordinates"
    );
    let released = router.route(&test, press(900.0, 900.0, false));
    assert_eq!(
        targets(&released),
        vec![EDITOR, EDITOR],
        "the release goes to the surface that held the press, which the pointer then leaves"
    );
    assert_eq!(inputs_for(&released, EDITOR)[1], Input::PointerLeft);
    assert_eq!(router.pressed(), None);
    assert_eq!(router.hovered(), Some(ROOT));
}

#[test]
fn hover_follows_the_surface_under_the_pointer() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
    assert_eq!(router.hovered(), Some(EDITOR));
    let moved = router.route(&test, Input::PointerMoved(pos2(150.0, 150.0)));
    assert_eq!(
        moved,
        vec![
            Routed::Input {
                target: EDITOR,
                input: Input::PointerLeft,
            },
            Routed::Input {
                target: CANVAS,
                input: Input::PointerMoved(pos2(50.0, 50.0)),
            },
        ]
    );
}

#[test]
fn a_press_focuses_its_surface_unless_it_never_takes_the_focus() {
    let mut test = canvas_with_editor().with(3, rect(800.0, 800.0, 100.0, 100.0), Some(ROOT));
    test.last().focus = false;
    let mut router = Router::new(ROOT);
    let pressed = router.route(&test, press(250.0, 250.0, true));
    assert_eq!(
        pressed[0],
        Routed::Input {
            target: EDITOR,
            input: Input::Focus(true),
        }
    );
    router.route(&test, press(250.0, 250.0, false));
    router.route(&test, press(850.0, 850.0, true));
    assert_eq!(
        router.focused(),
        Some(EDITOR),
        "a popup or the on-screen keyboard leaves the focus where it was"
    );
    router.route(&test, press(850.0, 850.0, false));
    let moved = router.focus(Some(CANVAS));
    assert_eq!(
        moved,
        vec![
            Routed::Input {
                target: EDITOR,
                input: Input::Focus(false),
            },
            Routed::Input {
                target: CANVAS,
                input: Input::Focus(true),
            },
        ],
        "the compositor moves the focus, telling both"
    );
}

#[test]
fn a_press_outside_a_surface_that_captures_presses_goes_to_its_placer() {
    let mut test = canvas_with_editor().with(3, rect(800.0, 800.0, 100.0, 100.0), Some(ROOT));
    test.last().captures = true;
    test.last().focus = false;
    let mut router = Router::new(ROOT);
    let pressed = router.route(&test, press(250.0, 250.0, true));
    assert_eq!(
        targets(&pressed),
        vec![ROOT, ROOT],
        "the compositor that placed the popup hears the press, to dismiss it, and nothing under it"
    );
}

#[test]
fn keys_go_to_the_focus_and_the_compositors_chords_go_to_it_first() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.intercept(vec![KeyChord::new(
        Key::Tab,
        Modifiers {
            alt: true,
            ..Modifiers::NONE
        },
    )]);
    router.route(&test, press(250.0, 250.0, true));
    router.route(&test, press(250.0, 250.0, false));
    assert_eq!(
        targets(&router.route(&test, key(Key::Tab, true))),
        vec![EDITOR]
    );
    router.route(&test, key(Key::Tab, false));
    router.route(
        &test,
        Input::Modifiers(Modifiers {
            alt: true,
            ..Modifiers::NONE
        }),
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::Tab, true))),
        vec![ROOT]
    );
    router.route(&test, Input::Modifiers(Modifiers::NONE));
    assert_eq!(
        targets(&router.route(&test, key(Key::Tab, false))),
        vec![ROOT],
        "a key's release follows its press"
    );
}

#[test]
fn a_wheel_over_an_editor_goes_to_the_outermost_camera_it_grants_the_gesture() {
    let mut test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
    let scrolled = |router: &mut Router<u32>, test: &Rects| {
        targets(&router.route(test, Input::Zoom(1.1)))
    };
    assert_eq!(scrolled(&mut router, &test), vec![EDITOR]);
    test.surfaces[2].gestures = Some(rect(200.0, 200.0, 200.0, 200.0));
    assert_eq!(scrolled(&mut router, &test), vec![CANVAS]);
    test.surfaces[1].gestures = Some(rect(100.0, 100.0, 600.0, 600.0));
    assert_eq!(
        scrolled(&mut router, &test),
        vec![ROOT],
        "claims go outermost first"
    );
}

#[test]
fn a_finger_landing_for_a_pinch_its_placer_claims_takes_the_earlier_fingers_with_it() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, touch(1, TouchPhase::Start, 250.0, 250.0));
    let landed = router.route(&test, touch(2, TouchPhase::Start, 150.0, 150.0));
    assert_eq!(
        landed,
        vec![
            Routed::Input {
                target: EDITOR,
                input: touch(1, TouchPhase::Cancel, 50.0, 50.0),
            },
            Routed::Input {
                target: CANVAS,
                input: touch(1, TouchPhase::Start, 150.0, 150.0),
            },
            Routed::Input {
                target: CANVAS,
                input: touch(2, TouchPhase::Start, 50.0, 50.0),
            },
        ],
        "a pinch with one finger on the canvas and one on the editor placed on it is the canvas's"
    );
    assert_eq!(
        targets(&router.route(&test, touch(1, TouchPhase::Move, 260.0, 260.0))),
        vec![CANVAS]
    );
}

#[test]
fn two_fingers_on_one_editor_stay_with_it() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, touch(1, TouchPhase::Start, 250.0, 250.0));
    assert_eq!(
        targets(&router.route(&test, touch(2, TouchPhase::Start, 300.0, 300.0))),
        vec![EDITOR]
    );
}

#[test]
fn a_held_press_handed_to_the_placer_ends_cancelled_in_the_editor() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    let handed = router.hand_over(&test, EDITOR);
    assert_eq!(
        handed,
        vec![
            Routed::Input {
                target: EDITOR,
                input: Input::PointersCancelled,
            },
            Routed::Input {
                target: CANVAS,
                input: press(150.0, 150.0, true),
            },
        ]
    );
    assert_eq!(
        targets(&router.route(&test, Input::PointerMoved(pos2(260.0, 260.0)))),
        vec![CANVAS]
    );
}

#[test]
fn only_the_focused_surface_holds_the_pointer_lock_and_loses_it_with_the_focus() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    assert!(!router.lock(EDITOR), "an unfocused surface is refused");
    router.focus(Some(EDITOR));
    assert!(router.lock(EDITOR));
    assert_eq!(
        targets(&router.route(&test, Input::PointerMotion(be_lib_geometry::vec2(3.0, 0.0)))),
        vec![EDITOR]
    );
    let moved = router.focus(Some(CANVAS));
    assert_eq!(moved[0], Routed::Input {
        target: EDITOR,
        input: Input::PointerUnlocked,
    });
    assert_eq!(router.locked(), None);
}

#[test]
fn a_drag_goes_over_the_surfaces_under_the_pointer_and_drops_on_the_one_under_the_release() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    let started = router.start_drag(&test, EDITOR);
    assert_eq!(
        started,
        vec![
            Routed::Input {
                target: EDITOR,
                input: Input::PointersCancelled,
            },
            Routed::Drag {
                target: EDITOR,
                drag: Drag::Over(pos2(50.0, 50.0)),
            },
        ]
    );
    let moved = router.route(&test, Input::PointerMoved(pos2(150.0, 150.0)));
    assert_eq!(
        moved,
        vec![
            Routed::Drag {
                target: EDITOR,
                drag: Drag::Left,
            },
            Routed::Drag {
                target: CANVAS,
                drag: Drag::Over(pos2(50.0, 50.0)),
            },
        ]
    );
    let dropped = router.route(&test, press(150.0, 150.0, false));
    assert_eq!(
        dropped[0],
        Routed::Drag {
            target: CANVAS,
            drag: Drag::Dropped {
                at: pos2(50.0, 50.0),
                from: EDITOR,
            },
        }
    );
}

#[test]
fn cancelled_pointers_end_the_press_the_pen_the_fingers_and_a_carry() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, touch(1, TouchPhase::Start, 150.0, 150.0));
    router.route(&test, press(250.0, 250.0, true));
    let cancelled = router.route(&test, Input::PointersCancelled);
    assert_eq!(
        cancelled,
        vec![
            Routed::Input {
                target: EDITOR,
                input: Input::PointersCancelled,
            },
            Routed::Input {
                target: CANVAS,
                input: Input::PointersCancelled,
            },
        ],
        "each surface holding a press hears once that it ended"
    );
    assert_eq!(router.pressed(), None);
    assert!(
        router
            .route(&test, touch(1, TouchPhase::Move, 160.0, 160.0))
            .is_empty(),
        "a cancelled finger goes nowhere"
    );
    router.route(&test, press(250.0, 250.0, true));
    router.start_drag(&test, EDITOR);
    let ended = router.route(&test, Input::PointersCancelled);
    assert_eq!(
        ended,
        vec![Routed::Drag {
            target: EDITOR,
            drag: Drag::Left,
        }],
        "a carry ends where it was, dropped nowhere"
    );
    assert!(
        router.route(&test, press(150.0, 150.0, false)).iter().all(
            |routed| !matches!(routed, Routed::Drag { drag: Drag::Dropped { .. }, .. })
        )
    );
}

#[test]
fn a_forgotten_surface_holds_nothing_and_hears_nothing() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    router.route(&test, key(Key::Tab, true));
    assert!(router.lock(EDITOR));
    router.forget(EDITOR);
    assert_eq!(
        (
            router.pressed(),
            router.focused(),
            router.hovered(),
            router.locked()
        ),
        (None, None, None, None)
    );
    assert!(router.route(&test, key(Key::Tab, false)).is_empty());
    assert!(
        router
            .route(&test, Input::PointerMotion(be_lib_geometry::vec2(1.0, 0.0)))
            .is_empty()
    );
    assert_eq!(
        targets(&router.route(&test, Input::PointerMoved(pos2(150.0, 150.0)))),
        vec![CANVAS],
        "the pointer it held is free again"
    );
}

#[test]
fn a_keys_release_follows_its_press_after_the_focus_moves() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.focus(Some(EDITOR));
    router.route(&test, key(Key::A, true));
    router.focus(Some(CANVAS));
    assert_eq!(
        targets(&router.route(&test, key(Key::A, false))),
        vec![EDITOR]
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::A, true))),
        vec![CANVAS],
        "the next press goes to the new focus"
    );
}

#[test]
fn a_pen_down_holds_its_surface_until_it_lifts() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    let pen = |phase: PenPhase, x: f32, y: f32| Input::Pen {
        phase,
        pos: pos2(x, y),
        pen: be_lib_ui_types::input::Pen {
            pressure: 0.5,
            tilt: be_lib_geometry::vec2(0.0, 0.0),
            eraser: false,
        },
    };
    let down = router.route(&test, pen(PenPhase::Down(PointerButton::Primary), 250.0, 250.0));
    assert_eq!(targets(&down), vec![EDITOR, EDITOR], "it focuses and hears the pen");
    assert_eq!(
        router.route(&test, pen(PenPhase::Move, 600.0, 600.0)),
        vec![Routed::Input {
            target: EDITOR,
            input: pen(PenPhase::Move, 400.0, 400.0),
        }]
    );
    assert_eq!(
        targets(&router.route(&test, pen(PenPhase::Up(PointerButton::Primary), 600.0, 600.0))),
        vec![EDITOR]
    );
    assert_eq!(
        targets(&router.route(&test, pen(PenPhase::Move, 600.0, 600.0))),
        vec![CANVAS],
        "once lifted it hovers what is under it"
    );
}

#[test]
fn a_scroll_and_a_touchpad_gesture_stay_with_the_surface_they_began_on() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
    assert_eq!(targets(&router.route(&test, Input::ScrollStarted)), vec![EDITOR]);
    router.route(&test, Input::PointerMoved(pos2(150.0, 150.0)));
    assert_eq!(
        targets(&router.route(
            &test,
            Input::Scroll(be_lib_ui_types::input::ScrollDelta::Pixels(
                be_lib_geometry::vec2(0.0, 4.0)
            ))
        )),
        vec![EDITOR],
        "a scroll that began over the editor stays with it"
    );
    assert_eq!(targets(&router.route(&test, Input::ScrollEnded)), vec![EDITOR]);
    assert_eq!(
        targets(&router.route(&test, Input::Touchpad(TouchpadGesture::PinchBegan(2)))),
        vec![CANVAS],
        "the next gesture begins where the pointer is now"
    );
    router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
    assert_eq!(
        targets(&router.route(&test, Input::Touchpad(TouchpadGesture::PinchEnded(false)))),
        vec![CANVAS]
    );
    assert_eq!(
        targets(&router.route(&test, Input::Touchpad(TouchpadGesture::HoldBegan(2)))),
        vec![EDITOR]
    );
}
