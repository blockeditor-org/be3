use super::*;

#[test]
fn a_keys_release_follows_its_press_after_the_focus_moves() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.focus(Some(EDITOR));
    router.route(&test, key(Key::A, true));
    router.focus(Some(CANVAS));
    assert_eq!(
        targets(&router.route(&test, key(Key::A, false))),
        vec![EDITOR]
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::A, true))),
        vec![CANVAS],
        "the next press goes to the new focus"
    );
}
