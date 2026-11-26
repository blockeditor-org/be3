use super::*;

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
