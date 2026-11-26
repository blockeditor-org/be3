use super::*;

#[test]
fn a_release_after_the_window_lost_the_keyboard_goes_nowhere() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.focus(Some(EDITOR));
    router.route(&test, key(Key::A, true));
    // The window loses the keyboard: the editor, still focused within it, lets go of its keys.
    assert_eq!(
        targets(&router.route(&test, Input::Focus(false))),
        vec![EDITOR]
    );
    assert!(
        router.route(&test, key(Key::A, false)).is_empty(),
        "the release of a key held when the window lost the keyboard goes nowhere"
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::B, false))),
        vec![EDITOR],
        "a release whose press the router never saw still goes to the focus"
    );
}
