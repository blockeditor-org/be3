use super::*;

#[test]
fn the_demo_leaves_a_pane_saying_nothing_is_open() {
    let mut test = demo(WIDE);
    let root = test.document().root().expect("the demo built a root");
    assert_eq!(
        showing(test.document(), root, "Nothing open"),
        0,
        "the demo opens with a page rather than the empty workspace"
    );

    test.click(&format!("dock.tab.{}.close", Page::Docking.tab().value()));
    test.frame(Vec::new());

    assert_eq!(
        showing(test.document(), root, "Nothing open"),
        1,
        "closing the last page leaves its pane saying there is nothing open"
    );
    open(&mut test, Page::Canvas);

    assert_eq!(
        showing(test.document(), root, "Nothing open"),
        0,
        "opening a page takes the place the empty workspace held"
    );
    assert!(
        showing(test.document(), root, "Canvas") >= 2,
        "the page it opened is the one the catalog named"
    );
}
