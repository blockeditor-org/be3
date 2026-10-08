use super::*;

#[test]
fn escape_leaves_the_window_switcher_without_switching() {
    let mut fixture = Fixture::with_windows(&[1, 2, 3]);
    let alt = Modifiers::ALT;
    let escape = KeyChord {
        key: Key::Escape,
        modifiers: alt,
    };
    assert!(
        !fixture.test.intercepted_keys().contains(&escape),
        "a program keeps Alt+Escape while nothing is being switched"
    );

    assert!(fixture.test.app_key(alt, Key::Tab));
    fixture.settle();
    assert!(fixture.test.app_key(alt, Key::Escape));
    fixture.settle();
    assert!(
        !fixture
            .test
            .shown(&format!("dock.switch.{}", window_tab(2)))
    );

    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
    assert!(
        fixture.focused_windows().is_empty(),
        "the window that had the keyboard keeps it"
    );
    assert!(!fixture.test.intercepted_keys().contains(&escape));
}
