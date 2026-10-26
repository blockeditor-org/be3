use std::time::Duration;

use crate::app::osd::OSD_DURATION;

use super::*;

#[test]
fn with_instant_motion_the_level_keeps_its_time_and_goes_without_fading() {
    let mut fixture = Fixture::new();
    fixture
        .test
        .set_preferences(block_editor_beui::Preferences {
            motion: block_editor_beui::Motion::Instant,
            ..Default::default()
        });
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
    fixture
        .test
        .advance(OSD_DURATION - Duration::from_millis(100));
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.osd.0"),
        "the level stays up as long as with motion"
    );

    fixture.test.advance(Duration::from_millis(200));
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.osd.0"),
        "and then goes at once rather than fading"
    );
}
