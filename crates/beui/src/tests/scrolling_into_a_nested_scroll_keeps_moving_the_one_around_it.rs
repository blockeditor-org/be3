use super::*;

#[test]
fn scrolling_into_a_nested_scroll_keeps_moving_the_one_around_it() {
    let mut harness = Harness::new(nested());
    harness.frame(Vec::new());
    let (inner, outer) = (harness.find("inner"), harness.find("outer"));
    let still = pos2(200.0, 190.0);

    harness.scroll(still, vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(outer), 20.0);
    assert!(harness.rect(inner).contains(still));

    harness.scroll(still, vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(inner), 0.0);
    assert_eq!(harness.document().scroll_offset(outer), 40.0);

    harness.advance(WHEEL_LATCH_TIMEOUT);
    harness.scroll(still, vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(inner), 20.0);
    assert_eq!(harness.document().scroll_offset(outer), 40.0);
}
