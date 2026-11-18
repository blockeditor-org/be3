use super::*;

#[test]
fn a_repeat_and_the_release_of_a_held_key_go_where_its_press_went() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.focus(Some(EDITOR));
    assert_eq!(
        targets(&router.route(&test, key(Key::A, true))),
        vec![EDITOR]
    );
    router.focus(Some(CANVAS));
    let repeat = Input::Key(Keystroke {
        key: Some(Key::A),
        pressed: true,
        repeat: true,
        evdev: None,
        text: None,
    });
    assert_eq!(
        targets(&router.route(&test, repeat)),
        vec![EDITOR],
        "a repeat goes to the editor that holds the key, not the canvas the focus moved to"
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::A, false))),
        vec![EDITOR],
        "and so does its release"
    );
}
