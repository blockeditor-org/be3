use super::*;

#[test]
fn a_virtual_scroll_only_builds_the_items_in_view() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let scroll = virtual_list(&built);
    let mut harness = Harness::new(scroll.document);

    harness.frame(Vec::new());

    let visible = (VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize;
    assert_eq!(*built.borrow(), (0..visible).collect::<Vec<usize>>());
    assert_eq!(harness.document.children(scroll.list).len(), visible);
}
