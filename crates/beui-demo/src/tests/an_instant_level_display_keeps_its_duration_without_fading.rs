use super::*;

#[test]
fn an_instant_level_display_keeps_its_duration_without_fading() {
    let mut test = level_display();
    test.context().set_motion(Motion::Instant);
    test.click("test.show");
    test.advance(OSD_DURATION - Duration::from_millis(100));
    assert!(test.shows("level-osd.0"));
    test.snapshot("instant_level_display");
    test.advance(Duration::from_millis(200));
    assert!(!test.shows("level-osd.0"));
    test.snapshot("instant_level_display_expired");
}
