use super::*;

#[test]
fn media_messages_round_trip() {
    let levels = MediaLevels {
        output: Some(MediaLevel {
            level: 0.45,
            muted: false,
        }),
        input: Some(MediaLevel {
            level: 1.0,
            muted: true,
        }),
        brightness: Some(0.3),
    };
    host_value_round_trips::<Media>(levels);
    host_value_round_trips::<Media>(MediaLevels::default());
    for request in [
        MediaRequest::StepVolume(-0.05),
        MediaRequest::SetVolume(0.7),
        MediaRequest::ToggleMute,
        MediaRequest::ToggleMicMute,
        MediaRequest::StepBrightness(0.05),
    ]
    .into_iter()
    .chain(PlayerCommand::ALL.map(MediaRequest::Player))
    {
        host_action_round_trips(request);
    }
}
