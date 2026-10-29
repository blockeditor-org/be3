use super::*;

#[test]
fn resizing_a_virtual_scroll_reuses_visible_items() {
    let _root = crate::reactive::RootScope::new();
    let built = create_signal(Vec::new());
    let scroll = virtual_list(built);
    let mut harness = Harness::new(scroll.document);
    harness.frame(Vec::new());
    let original = harness.document().children(scroll.list);

    built.update(|built| built.clear());
    harness.resize(vec2(VIEWPORT.x, VIRTUAL_ITEM_HEIGHT * 2.0));
    harness.frame(Vec::new());
    assert!(built.with_untracked(|built| built.is_empty()));
    assert_eq!(harness.document_mut().children(scroll.list), original[..2]);
    assert!(
        original[2..4]
            .iter()
            .all(|&id| harness.document().contains(id)),
        "a window's worth of the rows that left is kept"
    );

    harness.resize(VIEWPORT);
    harness.frame(Vec::new());
    let children = harness.document_mut().children(scroll.list);
    assert!(built.with_untracked(|built| built.is_empty()));
    assert_eq!(children, original);
}
