use super::*;

#[test]
fn a_fling_to_the_start_bounces_the_whole_sheet() {
    let _root = RootScope::new();
    let sheet = NodeRef::new();
    let closed = create_signal(0);
    let document = sheet_of_rows(0.9, sheet, closed);
    let mut harness = Harness::sized(document, vec2(400.0, 600.0));
    harness.settle();
    let sheet = sheet.get();
    harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 200.0));
    harness.settle();
    let handle = harness.find("sheet.handle");
    let top = harness.rect(handle).top();
    harness.finger_drag(&[pos2(200.0, 200.0), pos2(200.0, 260.0), pos2(200.0, 320.0)]);
    let mut bounced = false;
    for _ in 0..120 {
        harness.frame(Vec::new());
        if harness.rect(handle).top() > top + 1.0 {
            bounced = true;
            assert_eq!(harness.document().scroll_offset(sheet), 0.0);
            assert_eq!(harness.document().scroll_overscroll(sheet), 0.0);
            break;
        }
    }
    assert!(
        bounced,
        "the panel and grip move when the fling reaches the start"
    );
    harness.settle();
    assert_eq!(harness.rect(handle).top(), top);
    assert_eq!(closed.get_untracked(), 0);
}
