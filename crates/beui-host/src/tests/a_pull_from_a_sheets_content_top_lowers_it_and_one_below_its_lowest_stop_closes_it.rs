use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

/// A sheet is one scroll: a swipe up raises it to its top stop and then scrolls what it holds
/// under its grip; a swipe down scrolls it back, then lowers the sheet, and a flick down from
/// its lowest stop asks to close it.
#[test]
fn a_pull_from_a_sheets_content_top_lowers_it_and_one_below_its_lowest_stop_closes_it() {
    let _root = RootScope::new();
    let sheet = NodeRef::new();
    let closed = create_signal(0);
    let document = build_node(move || {
        view! {
            <ModalSheet
                @node_ref=&sheet
                open=true
                rest=0.5
                on_close={move || closed.update(|n| *n += 1)}
            >
                <Frame height=2000.0 />
            </ModalSheet>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 600.0));
    harness.settle();
    let handle = harness.find("sheet.handle");
    let offset = |harness: &Harness| harness.document().scroll_offset(sheet.get());
    assert_eq!(harness.rect(handle).top(), 300.0);

    harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 320.0));
    harness.settle();
    assert_eq!(harness.rect(handle).top(), 60.0, "raised to its top stop");
    assert_eq!(offset(&harness), 540.0);

    harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 200.0));
    harness.settle();
    let scrolled = offset(&harness);
    assert!(
        scrolled > 700.0,
        "then what it holds scrolls, to {scrolled}"
    );
    assert_eq!(
        harness.rect(handle).top(),
        60.0,
        "under its grip, which stays put"
    );

    harness.finger_drag_and_hold(pos2(200.0, 150.0), pos2(200.0, 550.0));
    harness.settle();
    assert!(
        offset(&harness) < scrolled,
        "a swipe down scrolls it back first"
    );
    harness.finger_drag_and_hold(pos2(200.0, 150.0), pos2(200.0, 550.0));
    harness.settle();
    assert!(
        harness.rect(handle).top() > 60.0,
        "and once at its start, lowers the sheet to a stop"
    );
    assert_eq!(closed.get_untracked(), 0);

    let from = harness.center(handle) + vec2(0.0, 60.0);
    harness.finger_drag(&[
        from,
        from + vec2(0.0, 40.0),
        from + vec2(0.0, 80.0),
        from + vec2(0.0, 120.0),
    ]);
    harness.settle();
    assert_eq!(
        closed.get_untracked(),
        1,
        "a flick down past its lowest stop asks to close it"
    );
    assert_eq!(
        harness.rect(handle).top(),
        300.0,
        "kept open by its owner, it rises back to where it rests"
    );
}
