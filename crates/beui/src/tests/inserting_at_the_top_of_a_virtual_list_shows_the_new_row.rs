use super::*;

#[test]
fn inserting_at_the_top_of_a_virtual_list_shows_the_new_row() {
    let _root = crate::reactive::RootScope::new();
    let built = create_signal(Vec::new());
    let scroll = keyed_virtual_list(built);
    let mut harness = Harness::new(scroll.document);
    harness.frame(Vec::new());
    let first = harness.document().children(scroll.list)[0];

    built.update(|built| built.clear());
    let mut keys = indices(VIRTUAL_ITEM_COUNT);
    keys.insert(0, VIRTUAL_ITEM_COUNT);
    with_installed(harness.document_mut(), |_| scroll.keys.set(keys));
    harness.frame(Vec::new());

    assert_eq!(built.get_untracked(), [VIRTUAL_ITEM_COUNT]);
    assert_eq!(harness.document().scroll_offset(scroll.scroll), 0.0);
    let rows = harness.document().children(scroll.list);
    assert_eq!(harness.rect(rows[0]).top(), 0.0);
    assert_eq!(rows[1], first);
    assert_eq!(harness.rect(first).top(), VIRTUAL_ITEM_HEIGHT);
}
