use super::*;

#[test]
fn resizing_a_virtual_scroll_reuses_visible_items() {
    let _root = crate::reactive::RootScope::new();
    let (built, set_built) = create_signal(Vec::new());
    let scroll = virtual_list(set_built);
    let mut harness = Harness::new(scroll.document);
    harness.frame(Vec::new());
    let original = harness.document.children(scroll.list);

    set_built.update(|built| built.clear());
    harness.viewport.y = VIRTUAL_ITEM_HEIGHT * 2.0;
    harness.frame(Vec::new());
    assert!(built.with_untracked(|built| built.is_empty()));
    assert_eq!(harness.document.children(scroll.list), original[..2]);
    assert!(
        original[2..]
            .iter()
            .all(|&id| !harness.document.contains(id))
    );

    harness.viewport = VIEWPORT;
    harness.frame(Vec::new());
    let children = harness.document.children(scroll.list);
    assert_eq!(
        built.get_untracked(),
        (2..original.len()).collect::<Vec<usize>>()
    );
    assert_eq!(children.len(), original.len());
    assert_eq!(children[..2], original[..2]);
}
