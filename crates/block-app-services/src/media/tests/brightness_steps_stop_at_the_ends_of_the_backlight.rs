use super::*;
use backlight::stepped;

#[test]
fn brightness_steps_stop_at_the_ends_of_the_backlight() {
    assert_eq!(stepped(100, 1000, BRIGHTNESS_STEP), 150);
    assert_eq!(stepped(400, 1000, -BRIGHTNESS_STEP), 350);
    assert_eq!(
        stepped(980, 1000, BRIGHTNESS_STEP),
        1000,
        "a step up stops at full brightness"
    );
    assert_eq!(
        stepped(30, 1000, -BRIGHTNESS_STEP),
        1,
        "a step down leaves the panel lit"
    );
    assert_eq!(
        stepped(3, 7, BRIGHTNESS_STEP),
        4,
        "a backlight with few levels still moves a level at a time"
    );
}
