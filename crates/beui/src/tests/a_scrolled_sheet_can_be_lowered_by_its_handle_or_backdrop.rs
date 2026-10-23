use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

#[test]
fn a_scrolled_sheet_can_be_lowered_by_its_handle_or_backdrop() {
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
    harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 200.0));
    harness.settle();
    let offset = harness.document().scroll_offset(sheet.get());
    assert!(offset > 0.0);
    let handle = harness.find("sheet.handle");
    let from = harness.center(handle);
    let before = harness.rect(handle).top();
    harness.drag(from, from + vec2(0.0, 160.0));
    harness.settle();
    assert_eq!(harness.rect(handle).top(), before);
    assert_eq!(closed.get_untracked(), 0);
    assert_eq!(harness.document().scroll_offset(sheet.get()), offset);
    let from = harness.center(handle);
    let before = harness.rect(handle).top();
    harness.touch(TouchPhase::Start, from);
    harness.touch(TouchPhase::Move, from + vec2(0.0, 40.0));
    assert!(harness.rect(handle).top() > before + 20.0);
    assert_eq!(harness.document().scroll_offset(sheet.get()), offset);
    harness.touch(TouchPhase::Cancel, from + vec2(0.0, 40.0));
    harness.settle();
    assert_eq!(closed.get_untracked(), 0);
    assert_eq!(harness.rect(handle).top(), before);
    harness.scroll_at(pos2(200.0, 400.0), vec2(0.0, -40.0), Modifiers::NONE);
    harness.settle();
    assert_eq!(harness.document().scroll_offset(sheet.get()), offset + 40.0);
    let outside = harness.find("sheet.outside");
    let from = harness.center(outside);
    harness.finger_drag_and_hold(from, from + vec2(0.0, 400.0));
    harness.settle();
    assert_eq!(closed.get_untracked(), 1);
    assert_eq!(harness.rect(handle).top(), before);
    let from = harness.center(handle);
    harness.finger_drag_and_hold(from, from + vec2(0.0, 500.0));
    harness.settle();
    assert_eq!(closed.get_untracked(), 2);
}
