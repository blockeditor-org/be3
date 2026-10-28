use std::time::Duration;

use crate::app::osd::OSD_DURATION;

use super::*;

#[test]
fn the_level_stays_up_while_keys_keep_coming_and_fades_rather_than_vanishing() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_watched::<MediaWatch>(&MediaLevels {
        output: Some(MediaLevel {
            level: 0.65,
            muted: false,
        }),
        input: None,
        brightness: None,
        ..MediaLevels::default()
    });
    assert!(fixture.test.app_key(Modifiers::NONE, Key::VolumeUp));
    fixture.settle();
    assert!(fixture.test.shown("desktop.osd.0"));

    fixture
        .test
        .advance(OSD_DURATION - Duration::from_millis(100));
    fixture.test.run();
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.osd.0"),
        "the level stays up for a while"
    );

    assert!(fixture.test.app_key(Modifiers::NONE, Key::VolumeUp));
    fixture.settle();
    fixture
        .test
        .advance(OSD_DURATION - Duration::from_millis(100));
    fixture.test.run();
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.osd.0"),
        "another key keeps the level up for as long again"
    );

    fixture.test.advance(Duration::from_millis(200));
    fixture.test.run();
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.osd.0"),
        "the level fades rather than vanishing"
    );
    fixture.test.snapshot("the_volume_fading");

    fixture.test.advance(Duration::from_secs(1));
    fixture.test.run();
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.osd.0"),
        "the level is gone once it has faded"
    );
}
