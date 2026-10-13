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
        outputs: vec![
            AudioOutput {
                id: "alsa_output.pci-0000_00_1f.3.analog-stereo".to_owned(),
                name: "Built-in Audio".to_owned(),
            },
            AudioOutput {
                id: "bluez_output.headphones".to_owned(),
                name: "Headphones".to_owned(),
            },
        ],
        default_output: Some("bluez_output.headphones".to_owned()),
    };
    host_value_round_trips::<Media>(levels);
    host_value_round_trips::<Media>(MediaLevels::default());
    for request in [
        MediaRequest::StepVolume(-0.05),
        MediaRequest::SetVolume(0.7),
        MediaRequest::ToggleMute,
        MediaRequest::SetMute(true),
        MediaRequest::ChooseOutput("bluez_output.headphones".to_owned()),
        MediaRequest::ToggleMicMute,
        MediaRequest::StepBrightness(0.05),
    ]
    .into_iter()
    .chain(PlayerCommand::ALL.map(MediaRequest::Player))
    {
        host_action_round_trips(request);
    }
}
