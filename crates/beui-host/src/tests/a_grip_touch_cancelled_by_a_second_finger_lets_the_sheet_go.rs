use super::*;
use crate::reactive::*;

#[test]
fn a_grip_touch_cancelled_by_a_second_finger_lets_the_sheet_go() {
    let _root = crate::reactive::RootScope::new();
    let sheet = NodeRef::new();
    let closed = create_signal(0);
    let document = sheet_of_rows(0.9, sheet, closed);
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
    let sheet = sheet.get();
    let handle = harness.center(harness.find("sheet.handle"));

    harness.finger(1, TouchPhase::Start, handle);
    harness.finger(2, TouchPhase::Start, pos2(200.0, 400.0));
    harness.finger(2, TouchPhase::End, pos2(200.0, 400.0));
    harness.finger(1, TouchPhase::End, handle);
    harness.settle();

    harness.scroll_at(pos2(200.0, 400.0), vec2(0.0, -100.0), Modifiers::NONE);
    assert_eq!(
        harness.document().scroll_offset(sheet),
        100.0,
        "a grip touch the second finger cancelled no longer holds the sheet, \
         so a wheel scrolls what it holds"
    );
}
