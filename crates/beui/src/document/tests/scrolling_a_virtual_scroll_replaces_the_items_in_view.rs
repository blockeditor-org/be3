use super::*;

#[test]
fn scrolling_a_virtual_scroll_replaces_the_items_in_view() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let scroll = virtual_list(&built);
    let mut harness = Harness::new(scroll.document);
    harness.frame(Vec::new());

    built.borrow_mut().clear();
    harness.document.set_scroll_offset(scroll.scroll, 1000.0);
    harness.frame(Vec::new());

    let visible = (VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize;
    let first = (1000.0 / VIRTUAL_ITEM_HEIGHT) as usize;
    assert_eq!(
        *built.borrow(),
        (first..first + visible).collect::<Vec<usize>>()
    );
    assert_eq!(harness.document.children(scroll.list).len(), visible);
}
