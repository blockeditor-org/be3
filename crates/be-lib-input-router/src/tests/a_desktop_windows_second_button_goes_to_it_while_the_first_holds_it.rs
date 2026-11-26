use super::*;

/// A desktop window (`desktop::follow`) or a win32 window's content: a second button pressed
/// while the first holds the window goes to it wherever the pointer is, and only the last
/// release lets go.
#[test]
fn a_desktop_windows_second_button_goes_to_it_while_the_first_holds_it() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    router.route(&test, Input::PointerMoved(pos2(900.0, 900.0)));
    assert_eq!(
        targets(&router.route(&test, button(PointerButton::Secondary, 900.0, 900.0, true))),
        vec![EDITOR]
    );
    assert_eq!(
        targets(&router.route(&test, press(900.0, 900.0, false))),
        vec![EDITOR],
        "the first release goes to it, and the second button still holds it"
    );
    assert_eq!(router.pressed(), Some(EDITOR));
    let released = router.route(&test, button(PointerButton::Secondary, 900.0, 900.0, false));
    assert_eq!(targets(&released), vec![EDITOR, EDITOR]);
    assert_eq!(inputs_for(&released, EDITOR)[1], Input::PointerLeft);
    assert_eq!(router.pressed(), None);
}
