use super::*;

#[test]
fn the_media_keys_ask_the_host_even_from_a_program_with_the_keyboard() {
    let mut fixture = Fixture::with_windows(&[1]);
    let intercepted = fixture.test.intercepted_keys();
    for (key, _) in BINDINGS {
        assert!(
            intercepted.contains(&KeyChord::new(key, Modifiers::NONE)),
            "the desktop takes {key:?} before the program that has the keyboard"
        );
    }
    for (key, _) in BINDINGS {
        assert!(fixture.test.app_key(Modifiers::NONE, key));
        fixture.settle();
    }
    let mut expected = vec![
        MediaRequest::StepVolume(VOLUME_STEP),
        MediaRequest::StepVolume(-VOLUME_STEP),
        MediaRequest::ToggleMute,
        MediaRequest::ToggleMicMute,
        MediaRequest::StepBrightness(BRIGHTNESS_STEP),
        MediaRequest::StepBrightness(-BRIGHTNESS_STEP),
    ];
    expected.extend(PlayerCommand::ALL.map(MediaRequest::Player));
    assert_eq!(fixture.test.take_actions::<MediaRequest>(), expected);
}
