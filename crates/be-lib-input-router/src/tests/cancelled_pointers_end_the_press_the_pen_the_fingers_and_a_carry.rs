use super::*;

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
        router
            .route(&test, press(150.0, 150.0, false))
            .iter()
            .all(|routed| !matches!(
                routed,
                Routed::Drag {
                    drag: Drag::Dropped { .. },
                    ..
                }
            ))
    );
}
