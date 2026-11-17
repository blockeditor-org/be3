use super::*;

/// beui's on-screen keyboard (beui-keyboard) is a catcher that never takes the focus: a press on
/// it is its own, and the keys it types go to the field that has the focus.
#[test]
fn the_on_screen_keyboard_takes_presses_while_keys_stay_with_the_focused_field() {
    const KEYBOARD: u32 = 3;
    let mut test = canvas_with_editor().with(KEYBOARD, rect(0.0, 800.0, 1000.0, 200.0), Some(ROOT));
    test.last().focus = false;
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    router.route(&test, press(250.0, 250.0, false));
    let pressed = router.route(&test, press(500.0, 900.0, true));
    assert_eq!(targets(&pressed), vec![EDITOR, KEYBOARD]);
    assert_eq!(inputs_for(&pressed, EDITOR), vec![Input::PointerLeft]);
    assert_eq!(router.focused(), Some(EDITOR));
    router.route(&test, press(500.0, 900.0, false));
    assert_eq!(
        targets(&router.route(&test, key(Key::A, true))),
        vec![EDITOR]
    );
    assert_eq!(
        targets(&router.route(&test, Input::Text("a".into()))),
        vec![EDITOR]
    );
}
