use super::*;

#[test]
fn scrolling_a_virtual_scroll_reuses_overlapping_items() {
    let _root = crate::reactive::RootScope::new();
    let built = create_signal(Vec::new());
    let scroll = virtual_list(built);
    let mut harness = Harness::new(scroll.document);
    harness.frame(Vec::new());
    let original = harness.document().children(scroll.list);
    let visible = original.len();

    built.update(|built| built.clear());
    harness
        .document_mut()
        .set_scroll_offset(scroll.scroll, VIRTUAL_ITEM_HEIGHT * 2.0);
    harness.frame(Vec::new());
    let lower = harness.document().children(scroll.list);
    assert_eq!(built.get_untracked(), vec![visible, visible + 1]);
    assert_eq!(lower[..visible - 2], original[2..]);
    assert!(
        original[..2]
            .iter()
            .all(|&id| harness.document().contains(id)),
        "the rows that just left are kept"
    );

    built.update(|built| built.clear());
    harness
        .document_mut()
        .set_scroll_offset(scroll.scroll, VIRTUAL_ITEM_HEIGHT);
    harness.frame(Vec::new());
    let upper = harness.document().children(scroll.list);
    assert_eq!(
        built.get_untracked(),
        Vec::<usize>::new(),
        "scrolling back shows the kept row rather than building it again"
    );
    assert_eq!(upper[0], original[1]);
    assert_eq!(upper[1..], lower[..visible - 1]);
    assert!(harness.document().contains(*lower.last().unwrap()));
}
