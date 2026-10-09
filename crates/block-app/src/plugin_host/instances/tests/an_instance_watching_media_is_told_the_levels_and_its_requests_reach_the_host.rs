use super::*;

use block_plugin_api::{LinuxMessage, MediaLevel, MediaLevels, MediaRequest, PlayerCommand};

fn media_sent(messages: &[Message]) -> Vec<MediaLevels> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Linux {
                instance,
                message: LinuxMessage::Media(media),
            }) if *instance == INSTANCE => Some(*media),
            _ => None,
        })
        .collect()
}

fn ask(instances: &mut Instances, request: MediaRequest) -> bool {
    instances.editor_message(EditorMessage::Linux {
        instance: INSTANCE,
        message: LinuxMessage::RequestMedia(request),
    })
}

#[test]
fn an_instance_watching_media_is_told_the_levels_and_its_requests_reach_the_host() {
    let levels = MediaLevels {
        output: Some(MediaLevel {
            level: 0.5,
            muted: false,
        }),
        input: None,
        brightness: Some(0.8),
    };
    let mut instances = placed();
    instances.next_screens(PASS);

    assert!(!instances.set_media(levels), "nobody is watching yet");
    assert!(!instances.watches_media());
    assert!(media_sent(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.editor_message(EditorMessage::Linux {
        instance: INSTANCE,
        message: LinuxMessage::WatchMedia,
    }));
    assert!(instances.watches_media());
    assert_eq!(media_sent(&instances.next_screens(PASS).opened), vec![levels]);
    assert!(media_sent(&instances.next_screens(PASS).opened).is_empty());
    let muted = MediaLevels {
        output: Some(MediaLevel {
            level: 0.5,
            muted: true,
        }),
        ..levels
    };
    assert!(instances.set_media(muted));
    assert_eq!(media_sent(&instances.next_screens(PASS).opened), vec![muted]);

    let asked = [
        MediaRequest::StepVolume(0.05),
        MediaRequest::SetVolume(0.3),
        MediaRequest::Player(PlayerCommand::Next),
    ];
    for request in asked {
        assert!(ask(&mut instances, request));
    }
    assert_eq!(instances.take_media_requests(INSTANCE), asked.to_vec());
    assert!(instances.take_media_requests(INSTANCE).is_empty());

    while ask(&mut instances, MediaRequest::ToggleMute) {}
    assert_eq!(
        instances.take_media_requests(INSTANCE).len(),
        MAX_MEDIA_REQUESTS,
        "requests nobody takes are held only up to a limit"
    );
    assert!(
        !instances.editor_message(EditorMessage::Linux {
            instance: INSTANCE,
            message: LinuxMessage::Media(levels),
        }),
        "only the host reports the levels"
    );
}
