use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

#[test]
fn a_cancelled_scrolled_sheet_dismissal_returns_before_a_later_drag_closes_it() {
    for backdrop in [false, true] {
        let _root = RootScope::new();
        let sheet = NodeRef::new();
        let open = create_signal(true);
        let document = build_node(move || {
            view! {
                <ModalSheet @node_ref=&sheet open on_close={move || open.set(false)}>
                    <Frame height=2000.0 />
                </ModalSheet>
            }
        });
        let mut harness = Harness::sized(document, vec2(400.0, 600.0));
        harness.settle();
        harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 200.0));
        harness.settle();
        let offset = harness.document().scroll_offset(sheet.get());
        let handle = harness.find("sheet.handle");
        let top = harness.rect(handle).top();
        let target = if backdrop {
            harness.find("sheet.outside")
        } else {
            handle
        };
        let from = harness.center(target);
        harness.touch(TouchPhase::Start, from);
        harness.touch(TouchPhase::Move, from + vec2(0.0, 400.0));
        assert!(harness.rect(handle).top() > top + 350.0);
        assert_eq!(harness.document().scroll_offset(sheet.get()), offset);
        harness.touch(TouchPhase::Cancel, from + vec2(0.0, 400.0));
        harness.settle();
        assert!(open.get_untracked());
        assert_eq!(harness.rect(handle).top(), top);
        assert_eq!(harness.document().scroll_offset(sheet.get()), offset);
        let from = harness.center(target);
        harness.finger_drag(&[from, from + vec2(0.0, 40.0), from + vec2(0.0, 80.0)]);
        harness.settle();
        assert!(
            !open.get_untracked(),
            "a short fast fling dismisses the sheet"
        );
        assert!(harness.document().find_test_id("sheet.handle").is_none());
    }
}
