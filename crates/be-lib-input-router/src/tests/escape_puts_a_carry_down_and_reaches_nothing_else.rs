use super::*;

#[test]
fn escape_puts_a_carry_down_and_reaches_nothing_else() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    router.route(&test, Input::Modifiers(Modifiers::CTRL));
    assert_eq!(
        router.modifiers(),
        Modifiers::CTRL,
        "a drop reads the keys held"
    );
    router.start_drag(&test, EDITOR);
    router.route(&test, Input::PointerMoved(pos2(150.0, 150.0)));
    assert!(router.carrying());

    // Escape puts it down: the canvas it was over hears it left, and the key reaches no surface.
    let escaped = router.route(&test, key(Key::Escape, true));
    assert_eq!(
        escaped,
        vec![Routed::Drag {
            target: CANVAS,
            drag: Drag::Left,
        }]
    );
    assert!(!router.carrying());
    assert!(router.route(&test, key(Key::Escape, false)).is_empty());
    // The release of the press that carried it drops nothing.
    let released = router.route(&test, press(150.0, 150.0, false));
    assert!(
        released
            .iter()
            .all(|routed| !matches!(routed, Routed::Drag { .. })),
        "{released:?}"
    );
}
