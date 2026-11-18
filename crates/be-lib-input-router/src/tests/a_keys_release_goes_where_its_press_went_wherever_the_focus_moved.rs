use super::*;

/// As beui holds a key for whatever took its press (guides/beui_keyboard.md, Holding): the
/// release of a key whose press moved the focus (an Escape that closed a dialog, a Tab) goes
/// where the press went, never to the surface the focus moved to, which heard no press.
#[test]
fn a_keys_release_goes_where_its_press_went_wherever_the_focus_moved() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.focus(Some(EDITOR));
    assert_eq!(
        targets(&router.route(&test, key(Key::A, true))),
        vec![EDITOR]
    );
    assert_eq!(
        router.focus(Some(CANVAS)),
        vec![
            Routed::Input {
                target: EDITOR,
                input: Input::Focus(false),
            },
            Routed::Input {
                target: CANVAS,
                input: Input::Focus(true),
            },
        ]
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::A, false))),
        vec![EDITOR],
        "the release goes to the editor its press went to"
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::B, false))),
        vec![CANVAS],
        "a release whose press the router never saw goes to the focus"
    );

    // A surface that went hears nothing more: the release of a key it held goes nowhere.
    assert_eq!(
        targets(&router.route(&test, key(Key::A, true))),
        vec![CANVAS]
    );
    router.forget(CANVAS);
    assert!(router.route(&test, key(Key::A, false)).is_empty());
}
