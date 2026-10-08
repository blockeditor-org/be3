use super::*;

#[test]
fn escaping_a_pick_clears_its_highlight() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);

    harness.toggle_inspector();
    harness.toggle_picking();
    harness.frame(vec![Event::PointerMoved(pos2(4.0, 4.0))]);
    assert!(harness.inspector().state.hovered.get().is_some());

    harness.key(Key::Escape, Modifiers::NONE);

    assert_eq!(harness.inspector().state.hovered.get(), None);
}
