use super::*;

#[test]
fn a_super_tap_toggles_the_launcher_even_from_a_program_with_the_keyboard() {
    let mut fixture = Fixture::with_windows(&[1]);
    assert!(
        fixture
            .test
            .intercepted_keys()
            .contains(&KeyChord::tap(Key::Logo)),
        "the desktop asks the host for a tap of Super"
    );

    assert!(fixture.test.app_tap(Key::Logo));
    fixture.settle();
    assert!(fixture.test.shown("launcher"));
    assert!(fixture.test.wants_keyboard());
    assert!(matches!(
        fixture.test.take_actions::<ProgramAction>()[..],
        [ProgramAction::List { .. }]
    ));

    assert!(fixture.test.app_tap(Key::Logo));
    fixture.settle();
    assert!(!fixture.test.shown("launcher"), "a second tap closes it");

    fixture.test.key_press(Key::Logo);
    fixture.settle();
    assert!(
        fixture.test.shown("launcher"),
        "a tap the desktop hears itself opens it too"
    );
}
