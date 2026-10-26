use super::*;
use beui::{PreferenceLayer, PreferenceSource};

#[test]
fn an_instant_level_display_keeps_its_duration_without_fading() {
    let mut test = level_display();
    test.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    test.click("test.show");
    test.advance(OSD_DURATION - Duration::from_millis(100));
    test.frame(Vec::new());
    assert!(test.shows("level-osd"));
    test.snapshot("instant_level_display");
    test.advance(Duration::from_millis(200));
    test.frame(Vec::new());
    assert!(!test.shows("level-osd"));
    test.snapshot("instant_level_display_expired");
}
