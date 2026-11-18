use super::*;

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
