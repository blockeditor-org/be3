use std::time::Duration;

use block_editor_beui::beui::styled::OSD_DURATION;
use block_plugin_api::EditorMessage;

use super::*;

#[test]
fn a_volume_key_shows_the_level_the_host_reports_until_it_fades() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        fixture.test.sent().iter().any(|message| matches!(
            message,
            EditorMessage::Linux {
                message: LinuxMessage::WatchMedia,
                ..
            }
        )),
        "the desktop asks the host for the levels"
    );
    fixture.test.linux(LinuxMessage::Media(MediaLevels {
        output: Some(MediaLevel {
            level: 0.65,
            muted: false,
        }),
        input: Some(MediaLevel {
            level: 1.0,
            muted: false,
        }),
        brightness: None,
    }));
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.osd.0"),
        "nothing shows unasked"
    );

    assert!(fixture.test.app_key(Modifiers::NONE, Key::VolumeUp));
    fixture.settle();
    fixture.test.linux(LinuxMessage::Media(MediaLevels {
        output: Some(MediaLevel {
            level: 0.7,
            muted: false,
        }),
        input: None,
        brightness: None,
    }));
    fixture.settle();
    assert!(fixture.test.shown("desktop.osd.0"), "the new volume shows");
    fixture.test.snapshot("a_volume_key_shows_the_volume");

    assert!(fixture.test.app_key(Modifiers::NONE, Key::BrightnessUp));
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.osd.0"),
        "a brightness the host has not reported shows nothing"
    );

    assert!(fixture.test.app_key(Modifiers::NONE, Key::VolumeMute));
    fixture.test.linux(LinuxMessage::Media(MediaLevels {
        output: Some(MediaLevel {
            level: 0.7,
            muted: true,
        }),
        input: None,
        brightness: None,
    }));
    fixture.settle();
    assert!(fixture.test.shown("desktop.osd.0"));
    fixture.test.snapshot("a_mute_key_shows_the_volume_muted");

    fixture.test.advance(OSD_DURATION + Duration::from_secs(1));
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.osd.0"),
        "the level fades away on its own"
    );
}
