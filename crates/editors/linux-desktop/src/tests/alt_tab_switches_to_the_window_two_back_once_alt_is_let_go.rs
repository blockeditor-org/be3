use super::*;

#[test]
fn alt_tab_switches_to_the_window_two_back_once_alt_is_let_go() {
    let mut fixture = Fixture::with_windows(&[1, 2, 3]);
    let alt = Modifiers::ALT;
    assert!(
        fixture.test.intercepted_keys().contains(&KeyChord {
            key: Key::Tab,
            modifiers: alt
        }),
        "the desktop asks the host for Alt+Tab, even from the program that has the keyboard"
    );

    assert!(fixture.test.app_key(alt, Key::Tab));
    fixture.settle();
    assert!(fixture.test.app_key(alt, Key::Tab));
    fixture.settle();
    assert!(
        fixture
            .test
            .shown(&format!("dock.switch.{}", window_tab(1))),
        "the switcher lists the windows while Alt is held"
    );
    fixture
        .test
        .snapshot("alt_tab_switches_to_the_window_two_back_once_alt_is_let_go");
    assert!(
        fixture.focused_windows().is_empty(),
        "nothing is focused while Alt is held"
    );

    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(1)],
        "letting go of Alt gives the window two back the keyboard"
    );
    assert!(
        !fixture
            .test
            .shown(&format!("dock.switch.{}", window_tab(1)))
    );
}
