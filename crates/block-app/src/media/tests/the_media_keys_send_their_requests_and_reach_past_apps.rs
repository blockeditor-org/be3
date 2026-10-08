use super::*;

#[test]
fn the_media_keys_send_their_requests_and_reach_past_apps() {
    let mut harness = Harness::new();
    assert!(
        harness.actions.iter().all(Action::intercepts),
        "every media key is taken before a focused program or plugin sees it"
    );
    for (key, _) in BINDINGS {
        harness.press(key);
    }
    assert_eq!(
        harness.sent(),
        vec![
            Sent::Audio(AudioRequest::Volume(VOLUME_STEP)),
            Sent::Audio(AudioRequest::Volume(-VOLUME_STEP)),
            Sent::Audio(AudioRequest::ToggleMute),
            Sent::Audio(AudioRequest::ToggleMicMute),
            Sent::Brightness(BRIGHTNESS_STEP),
            Sent::Brightness(-BRIGHTNESS_STEP),
            Sent::Player(PlayerRequest::PlayPause),
            Sent::Player(PlayerRequest::Next),
            Sent::Player(PlayerRequest::Previous),
            Sent::Player(PlayerRequest::Stop),
        ]
    );
}
