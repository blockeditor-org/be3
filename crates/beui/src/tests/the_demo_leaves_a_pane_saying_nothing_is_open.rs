#![allow(dead_code)]

use super::*;

include!("../../examples/demo.rs");

#[test]
fn the_demo_leaves_a_pane_saying_nothing_is_open() {
    let mut harness = Harness::sized(DemoApp::new().document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the demo built a root");
    assert!(
        text_within(harness.document(), root, "Nothing open").is_none(),
        "the demo opens with a page rather than the empty workspace"
    );
    let close = harness
        .document()
        .find_test_id(&format!("dock.tab.{}.close", Page::Docking.tab().value()))
        .expect("the page can be closed");

    harness.click(harness.center(close));
    harness.frame(Vec::new());

    assert!(
        text_within(harness.document(), root, "Nothing open").is_some(),
        "closing the last page leaves its pane saying there is nothing open"
    );
    open_demo_page(&mut harness, "Canvas");

    assert!(
        text_within(harness.document(), root, "Nothing open").is_none(),
        "opening a page takes the place the empty workspace held"
    );
    assert!(
        text_within(harness.document(), root, "Canvas").is_some(),
        "the page it opened is the one the catalog named"
    );
}
