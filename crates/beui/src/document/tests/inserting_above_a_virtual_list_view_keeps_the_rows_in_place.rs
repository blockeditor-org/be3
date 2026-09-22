use super::*;

const INSERTED_AT: usize = 10;

#[test]
fn inserting_above_a_virtual_list_view_keeps_the_rows_in_place() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let scroll = keyed_virtual_list(&built);
    let mut harness = Harness::new(scroll.document);
    harness.document.set_scroll_offset(scroll.scroll, 1000.0);
    harness.frame(Vec::new());
    let rows = harness.document.children(scroll.list);
    let tops: Vec<f32> = rows.iter().map(|&row| harness.rect(row).top()).collect();

    built.borrow_mut().clear();
    let mut keys = indices(VIRTUAL_ITEM_COUNT);
    keys.insert(INSERTED_AT, VIRTUAL_ITEM_COUNT);
    with_installed(harness.document_mut(), |_| scroll.set_keys.set(keys));
    harness.frame(Vec::new());

    assert!(built.borrow().is_empty());
    assert_eq!(harness.document.children(scroll.list), rows);
    let moved: Vec<f32> = rows.iter().map(|&row| harness.rect(row).top()).collect();
    assert_eq!(moved, tops);
    assert_eq!(
        harness.document.scroll_offset(scroll.scroll),
        1000.0 + VIRTUAL_ITEM_HEIGHT
    );
}
