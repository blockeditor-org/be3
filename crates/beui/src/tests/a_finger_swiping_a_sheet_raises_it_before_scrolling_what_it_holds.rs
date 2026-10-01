use super::*;
use crate::reactive::NodeRef;

#[test]
fn a_finger_swiping_a_sheet_raises_it_before_scrolling_what_it_holds() {
    let sheet = NodeRef::new();
    let closed = Rc::new(Cell::new(0));
    let document = sheet_of_rows(0.5, &sheet, &closed);
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
    let sheet = sheet.get();
    let height = |harness: &Harness| harness.rect(sheet).height();
    let scrolled = |harness: &Harness| harness.document().scroll_offset(sheet);
    assert_eq!(height(&harness), 300.0, "a sheet opens at its resting stop");

    harness.finger_drag_and_hold(pos2(200.0, 450.0), pos2(200.0, 250.0));
    harness.settle();
    assert_eq!(
        height(&harness),
        540.0,
        "a swipe up on what the sheet holds raises the sheet to the top stop"
    );
    assert_eq!(
        scrolled(&harness),
        0.0,
        "and leaves its content where it was"
    );

    harness.finger_drag_and_hold(pos2(200.0, 300.0), pos2(200.0, 100.0));
    harness.settle();
    assert_eq!(height(&harness), 540.0, "at the top the sheet stays put");
    assert!(
        scrolled(&harness) > 180.0 && scrolled(&harness) <= 200.0,
        "and the swipe scrolls its content instead, by about as far as the \
         finger went past the touch slop, to {}",
        scrolled(&harness)
    );

    harness.finger_drag_and_hold(pos2(200.0, 150.0), pos2(200.0, 550.0));
    harness.settle();
    assert_eq!(
        scrolled(&harness),
        0.0,
        "a swipe down scrolls the content back to its start first"
    );
    assert_eq!(
        height(&harness),
        300.0,
        "and then lowers the sheet, which settles on the nearest stop"
    );
    assert_eq!(closed.get(), 0);
}
