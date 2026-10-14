use super::*;

#[test]
fn scrolling_a_virtual_scroll_reuses_overlapping_items() {
    let _root = crate::reactive::RootScope::new();
    let (built, set_built) = create_signal(Vec::new());
    let scroll = virtual_list(set_built);
    let mut harness = Harness::new(scroll.document);
    harness.frame(Vec::new());
    let original = harness.document.children(scroll.list);
    let visible = original.len();

    set_built.update(|built| built.clear());
    harness
        .document
        .set_scroll_offset(scroll.scroll, VIRTUAL_ITEM_HEIGHT * 2.0);
    harness.frame(Vec::new());
    let lower = harness.document.children(scroll.list);
    assert_eq!(built.get_untracked(), vec![visible, visible + 1]);
    assert_eq!(lower[..visible - 2], original[2..]);
    assert!(
        original[..2]
            .iter()
            .all(|&id| !harness.document.contains(id))
    );

    set_built.update(|built| built.clear());
    harness
        .document
        .set_scroll_offset(scroll.scroll, VIRTUAL_ITEM_HEIGHT);
    harness.frame(Vec::new());
    let upper = harness.document.children(scroll.list);
    assert_eq!(built.get_untracked(), vec![1]);
    assert_eq!(upper[1..], lower[..visible - 1]);
    assert!(!harness.document.contains(*lower.last().unwrap()));
}
