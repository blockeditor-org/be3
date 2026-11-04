use std::time::Duration;

use crate::root::osd::OSD_DURATION;

use super::*;

#[test]
fn a_volume_key_shows_the_level_the_host_reports_until_it_fades() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        fixture.test.watches::<MediaWatch>(),
        "the desktop asks the host for the levels"
    );
    fixture.test.set_watched::<MediaWatch>(&MediaLevels {
        output: Some(MediaLevel {
            level: 0.65,
            muted: false,
        }),
        input: Some(MediaLevel {
            level: 1.0,
            muted: false,
        }),
        brightness: None,
        ..MediaLevels::default()
    });
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.osd.0"),
        "nothing shows unasked"
    );

    assert!(fixture.test.host_key(Modifiers::NONE, Key::VolumeUp));
    fixture.settle();
    fixture.test.set_watched::<MediaWatch>(&MediaLevels {
        output: Some(MediaLevel {
            level: 0.7,
            muted: false,
        }),
        input: None,
        brightness: None,
        ..MediaLevels::default()
    });
    fixture.settle();
    assert!(fixture.test.shown("desktop.osd.0"), "the new volume shows");
    let card = fixture.test.rect_of("desktop.osd.0");
    assert!(
        fixture
            .test
            .occluders()
            .iter()
            .any(|occluder| occluder.rect == card),
        "the level covers the windows under its card"
    );
    assert!(
        fixture
            .test
            .occluders()
            .iter()
            .all(|occluder| !occluder.rect.contains_rect(card) || occluder.rect == card),
        "and nothing around the card, so windows show through its margin"
    );
    fixture.test.snapshot("a_volume_key_shows_the_volume");

    assert!(fixture.test.host_key(Modifiers::NONE, Key::BrightnessUp));
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.osd.0") && fixture.says("No backlight"),
        "a brightness key with no backlight to change says so"
    );

    assert!(fixture.test.host_key(Modifiers::NONE, Key::VolumeMute));
    fixture.test.set_watched::<MediaWatch>(&MediaLevels {
        output: Some(MediaLevel {
            level: 0.7,
            muted: true,
        }),
        input: None,
        brightness: None,
        ..MediaLevels::default()
    });
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
