use super::*;

const REMOVED_AT: usize = 3;

#[test]
fn removing_from_a_virtual_list_view_disposes_only_that_row() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let scroll = keyed_virtual_list(&built);
    let mut harness = Harness::new(scroll.document);
    harness.frame(Vec::new());
    let rows = harness.document.children(scroll.list);

    built.borrow_mut().clear();
    let mut keys = indices(VIRTUAL_ITEM_COUNT);
    keys.remove(REMOVED_AT);
    with_installed(harness.document_mut(), |_| scroll.set_keys.set(keys));
    harness.frame(Vec::new());

    assert!(!harness.document.contains(rows[REMOVED_AT]));
    assert_eq!(*built.borrow(), vec![rows.len()]);
    let after = harness.document.children(scroll.list);
    assert_eq!(after[..REMOVED_AT], rows[..REMOVED_AT]);
    assert_eq!(after[REMOVED_AT..rows.len() - 1], rows[REMOVED_AT + 1..]);
    assert_eq!(
        harness.rect(after[REMOVED_AT]).top(),
        REMOVED_AT as f32 * VIRTUAL_ITEM_HEIGHT
    );
}
