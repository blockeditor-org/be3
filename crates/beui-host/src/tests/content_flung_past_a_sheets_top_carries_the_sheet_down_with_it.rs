use super::*;
use crate::reactive::*;

/// A fling down through what a sheet holds that reaches the content's top goes on in the
/// sheet: the speed left over lowers it, rather than the content stopping at its top.
#[test]
fn content_flung_past_a_sheets_top_carries_the_sheet_down_with_it() {
    let _root = crate::reactive::RootScope::new();
    let sheet = NodeRef::new();
    let closed = create_signal(0);
    let document = sheet_of_rows(0.9, sheet, closed);
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
    let sheet = sheet.get();
    assert_eq!(sheet_raised(&harness, sheet), 540.0);

    for _ in 0..2 {
        harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 300.0));
        harness.settle();
    }
    let scrolled = sheet_scrolled(&harness, sheet);
    assert!(scrolled > 300.0, "what it holds scrolls, to {scrolled}");

    harness.finger_drag(&[
        pos2(200.0, 150.0),
        pos2(200.0, 230.0),
        pos2(200.0, 310.0),
        pos2(200.0, 390.0),
    ]);
    assert_eq!(
        sheet_raised(&harness, sheet),
        540.0,
        "let go before what it holds is back at its top"
    );
    let mut lowest = sheet_raised(&harness, sheet);
    for _ in 0..120 {
        harness.frame(Vec::new());
        lowest = lowest.min(sheet_raised(&harness, sheet));
    }
    assert!(
        lowest < 500.0,
        "the fling reaches the content's top and lowers the sheet, to {lowest}"
    );
    harness.settle();
    assert_eq!(sheet_scrolled(&harness, sheet), 0.0);
    let rest = sheet_raised(&harness, sheet);
    assert!(
        [180.0, 300.0, 540.0].contains(&rest),
        "and it comes to rest on a stop, at {rest}"
    );
}
