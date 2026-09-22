use super::*;

const INSERTED_AT: usize = 3;

#[test]
fn inserting_into_a_virtual_list_view_builds_only_the_new_row() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let scroll = keyed_virtual_list(&built);
    let mut harness = Harness::new(scroll.document);
    harness.frame(Vec::new());
    let rows = harness.document.children(scroll.list);

    built.borrow_mut().clear();
    let mut keys = indices(VIRTUAL_ITEM_COUNT);
    keys.insert(INSERTED_AT, VIRTUAL_ITEM_COUNT);
    with_installed(harness.document_mut(), |_| scroll.set_keys.set(keys));
    harness.frame(Vec::new());

    assert_eq!(*built.borrow(), vec![VIRTUAL_ITEM_COUNT]);
    let after = harness.document.children(scroll.list);
    assert_eq!(after.len(), rows.len());
    assert_eq!(after[..INSERTED_AT], rows[..INSERTED_AT]);
    assert_eq!(after[INSERTED_AT + 1..], rows[INSERTED_AT..rows.len() - 1]);
    assert!(!harness.document.contains(*rows.last().unwrap()));
    assert_eq!(
        harness.rect(after[INSERTED_AT + 1]).top(),
        (INSERTED_AT + 1) as f32 * VIRTUAL_ITEM_HEIGHT
    );
}
