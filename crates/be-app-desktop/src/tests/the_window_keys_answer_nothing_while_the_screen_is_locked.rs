use be_sdk_beui::ScreenLockedWatch;

use super::*;

#[test]
fn the_window_keys_answer_nothing_while_the_screen_is_locked() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    assert!(
        fixture
            .test
            .intercepted_keys()
            .contains(&KeyChord::new(Key::F4, Modifiers::ALT))
    );
    fixture.test.set_watched::<ScreenLockedWatch>(&true);
    fixture.settle();
    for (modifiers, key) in [
        (Modifiers::ALT, Key::F4),
        (Modifiers::LOGO, Key::ArrowLeft),
        (Modifiers::LOGO, Key::ArrowUp),
        (Modifiers::LOGO, Key::ArrowDown),
        (Modifiers::LOGO, Key::D),
    ] {
        assert!(
            !fixture.test.host_key(modifiers, key),
            "{modifiers:?}+{key:?} is not taken while the screen is locked"
        );
        fixture.test.key(key, modifiers);
        fixture.settle();
    }
    assert!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::window)
            .is_empty(),
        "nor does anything to a window"
    );
}
