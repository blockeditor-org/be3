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
        fixture.closed_windows().is_empty()
            && fixture.window_rect(1).is_some()
            && fixture.window_rect(2).is_some(),
        "nor does anything to a window"
    );
}
