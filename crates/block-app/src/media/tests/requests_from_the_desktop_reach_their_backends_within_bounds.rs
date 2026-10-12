use super::*;

#[test]
fn requests_from_the_desktop_reach_their_backends_within_bounds() {
    let (media, recorder, _events) = recorded();
    let requests = [
        MediaRequest::StepVolume(VOLUME_STEP),
        MediaRequest::StepVolume(-VOLUME_STEP),
        MediaRequest::SetVolume(0.7),
        MediaRequest::ToggleMute,
        MediaRequest::SetMute(true),
        MediaRequest::ChooseOutput("headphones".to_owned()),
        MediaRequest::ToggleMicMute,
        MediaRequest::StepBrightness(BRIGHTNESS_STEP),
        MediaRequest::StepBrightness(-BRIGHTNESS_STEP),
    ];
    for request in requests {
        media.request(request);
    }
    for command in PlayerCommand::ALL {
        media.request(MediaRequest::Player(command));
    }
    assert_eq!(
        recorder.sent(),
        vec![
            Sent::Audio(AudioRequest::Step(VOLUME_STEP)),
            Sent::Audio(AudioRequest::Step(-VOLUME_STEP)),
            Sent::Audio(AudioRequest::Set(0.7)),
            Sent::Audio(AudioRequest::ToggleMute),
            Sent::Audio(AudioRequest::SetMute(true)),
            Sent::Audio(AudioRequest::Choose("headphones".to_owned())),
            Sent::Audio(AudioRequest::ToggleMicMute),
            Sent::Brightness(BRIGHTNESS_STEP),
            Sent::Brightness(-BRIGHTNESS_STEP),
            Sent::Player(PlayerCommand::PlayPause),
            Sent::Player(PlayerCommand::Next),
            Sent::Player(PlayerCommand::Previous),
            Sent::Player(PlayerCommand::Stop),
        ]
    );

    for request in [
        MediaRequest::SetVolume(4.0),
        MediaRequest::SetVolume(-1.0),
        MediaRequest::SetVolume(f32::NAN),
        MediaRequest::StepVolume(f32::INFINITY),
        MediaRequest::StepVolume(0.0),
        MediaRequest::StepBrightness(-9.0),
        MediaRequest::ChooseOutput(String::new()),
        MediaRequest::ChooseOutput("head\0phones".to_owned()),
    ] {
        media.request(request);
    }
    assert_eq!(
        recorder.sent(),
        vec![
            Sent::Audio(AudioRequest::Set(1.0)),
            Sent::Audio(AudioRequest::Set(0.0)),
            Sent::Brightness(-1.0),
        ],
        "a plugin's numbers are kept within bounds, and numbers and names that are no such thing are dropped"
    );
}
