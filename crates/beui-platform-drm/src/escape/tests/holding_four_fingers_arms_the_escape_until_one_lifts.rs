use super::*;

#[test]
fn holding_four_fingers_arms_the_escape_until_one_lifts() {
    let mut escape = Escape::default();
    for finger in 0..3 {
        assert_eq!(escape.touch(finger, true), None);
    }
    assert_eq!(
        escape.touch(1, true),
        None,
        "a finger that moves is still one finger"
    );
    assert_eq!(
        escape.touch(3, true),
        Some(true),
        "the fourth finger arms it"
    );
    assert_eq!(escape.touch(4, true), None, "a fifth keeps it armed");
    assert_eq!(escape.touch(4, false), None);
    assert_eq!(
        escape.touch(0, false),
        Some(false),
        "lifting any of the four disarms it"
    );
    assert_eq!(
        escape.touch(0, true),
        Some(true),
        "putting it back arms it again"
    );
}
