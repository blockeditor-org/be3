use super::*;

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
