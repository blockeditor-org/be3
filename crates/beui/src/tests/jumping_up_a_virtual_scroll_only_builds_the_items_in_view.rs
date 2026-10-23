use super::*;

#[test]
fn jumping_up_a_virtual_scroll_only_builds_the_items_in_view() {
    let _root = crate::reactive::RootScope::new();
    let built = create_signal(Vec::new());
    let scroll = virtual_list(built);
    let mut harness = Harness::new(scroll.document);
    harness
        .document_mut()
        .set_scroll_offset(scroll.scroll, f32::MAX);
    harness.frame(Vec::new());

    built.update(|built| built.clear());
    harness
        .document_mut()
        .set_scroll_offset(scroll.scroll, VIRTUAL_ITEM_HEIGHT / 2.0);
    harness.frame(Vec::new());

    let visible = (VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize + 1;
    assert_eq!(built.get_untracked(), (0..visible).collect::<Vec<usize>>());
    let children = harness.document().children(scroll.list);
    assert_eq!(children.len(), visible);
    assert_eq!(harness.rect(children[0]).top(), -VIRTUAL_ITEM_HEIGHT / 2.0);
    assert!(harness.rect(*children.last().unwrap()).bottom() >= VIEWPORT.y);

    built.update(|built| built.clear());
    harness.frame(Vec::new());
    assert!(built.with_untracked(|built| built.is_empty()));
    assert_eq!(harness.document().children(scroll.list), children);
}
