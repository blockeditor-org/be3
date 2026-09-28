use super::*;

#[test]
fn moving_the_pointer_ends_the_wheel_latch() {
    let mut harness = Harness::new(nested());
    harness.frame(Vec::new());
    let (inner, outer) = (harness.find("inner"), harness.find("outer"));

    harness.scroll(pos2(200.0, 190.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(outer), 20.0);

    harness.scroll(pos2(200.0, 250.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().scroll_offset(inner),
        20.0,
        "the wheel scrolls what the pointer moved onto, however soon after the last turn"
    );
    assert_eq!(harness.document().scroll_offset(outer), 20.0);
}
