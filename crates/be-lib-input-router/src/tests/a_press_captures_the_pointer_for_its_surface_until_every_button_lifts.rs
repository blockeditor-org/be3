use super::*;

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
