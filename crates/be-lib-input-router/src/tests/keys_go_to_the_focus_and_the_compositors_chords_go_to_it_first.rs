use super::*;

#[test]
fn keys_go_to_the_focus_and_the_compositors_chords_go_to_it_first() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.intercept(vec![KeyChord::new(
        Key::Tab,
        Modifiers {
            alt: true,
            ..Modifiers::NONE
        },
    )]);
    router.route(&test, press(250.0, 250.0, true));
    router.route(&test, press(250.0, 250.0, false));
    assert_eq!(
        targets(&router.route(&test, key(Key::Tab, true))),
        vec![EDITOR]
    );
    router.route(&test, key(Key::Tab, false));
    router.route(
        &test,
        Input::Modifiers(Modifiers {
            alt: true,
            ..Modifiers::NONE
        }),
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::Tab, true))),
        vec![ROOT]
    );
    router.route(&test, Input::Modifiers(Modifiers::NONE));
    assert_eq!(
        targets(&router.route(&test, key(Key::Tab, false))),
        vec![ROOT],
        "a key's release follows its press"
    );
}
