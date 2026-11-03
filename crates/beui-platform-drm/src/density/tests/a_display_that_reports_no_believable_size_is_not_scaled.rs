use super::*;

#[test]
fn a_display_that_reports_no_believable_size_is_not_scaled() {
    assert_eq!(natural_scale((1920, 1080), None), 1.0, "no size at all");
    assert_eq!(
        natural_scale((1920, 1080), Some((0, 0))),
        1.0,
        "a projector reports zero"
    );
    assert_eq!(
        natural_scale((1920, 1080), Some((16, 9))),
        1.0,
        "an aspect ratio, not a size"
    );
    assert_eq!(
        natural_scale((7680, 4320), Some((60, 40))),
        3.0,
        "the scale stops at 3"
    );
}
