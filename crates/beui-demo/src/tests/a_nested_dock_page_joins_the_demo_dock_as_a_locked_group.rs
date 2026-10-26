use super::*;

#[test]
fn a_nested_dock_page_joins_the_demo_dock_as_a_locked_group() {
    let mut test = demo(WIDE);
    open(&mut test, Page::NestedDocks);
    test.frame(Vec::new());
    let document = test.document();
    assert!(
        document.find_test_id("dock.grip.views").is_none(),
        "the page's dock draws no pane of its own"
    );
    let views = format!("dock.grip.joined:{}/views", tab(Page::NestedDocks));
    assert!(
        document.find_test_id(&views).is_some(),
        "the demo's dock draws the page's panes in a group where the page's tab was"
    );
    let root = document.root().expect("the demo built a root");
    for title in ["Nested docks", "Outline", "Preview", "Notes"] {
        assert!(
            showing(document, root, title) >= 1,
            "the demo shows {title}"
        );
    }
    test.snapshot("a_nested_dock_page_joins_the_demo_dock_as_a_locked_group");

    open(&mut test, Page::Text);
    test.frame(Vec::new());
    assert!(
        test.document().find_test_id(&views).is_none(),
        "showing another page in the tab ends the group"
    );
}
