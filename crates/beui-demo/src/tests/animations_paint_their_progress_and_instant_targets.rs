use super::*;
use beui::reactive::Motion;
use std::time::Duration;

#[test]
fn animations_paint_their_progress_and_instant_targets() {
    let mut test = alone(WIDE, Page::Frames);
    test.click("animation.retarget");
    test.advance(Duration::from_millis(100));
    test.frame(Vec::new());
    test.snapshot("animation_progress");
    test.click("animation.retarget");
    test.advance(Duration::from_millis(50));
    test.frame(Vec::new());
    test.snapshot("animation_retargeted");
    test.click("animation.stop");
    test.snapshot("animation_interrupted");
    test.click("animation.instant");
    assert_eq!(test.context().motion(), Motion::Instant);
    test.click("animation.retarget");
    test.snapshot("animation_instant");
}
