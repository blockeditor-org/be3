use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

#[test]
fn a_backdrop_drag_expands_an_unscrolled_half_open_sheet() {
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
    assert_eq!(harness.rect(handle).top(), 300.0);
    harness.finger_drag_and_hold(pos2(200.0, 270.0), pos2(200.0, 40.0));
    harness.settle();
    assert_eq!(harness.rect(handle).top(), 60.0);
    assert_eq!(harness.document().scroll_offset(sheet.get()), 0.0);
    let from = harness.center(handle);
    harness.finger_drag_and_hold(from, from + vec2(0.0, 220.0));
    harness.settle();
    assert_eq!(harness.rect(handle).top(), 300.0);
    assert_eq!(harness.document().scroll_offset(sheet.get()), 0.0);
    assert_eq!(closed.get_untracked(), 0);
}
