#![allow(dead_code)]

use super::*;

include!("../../../examples/dock.rs");

#[test]
fn the_dock_demo_leaves_a_tab_saying_nothing_is_open() {
    let mut harness = Harness::sized(DockDemo::new().document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the demo built a root");
    assert!(
        text_within(harness.document(), root, "Nothing open").is_none(),
        "the demo opens with a paper rather than the empty workspace"
    );
    let close = harness
        .document()
        .find_test_id(&format!("dock.tab.{}.close", PAPERS[0].0))
        .expect("the paper can be closed");

    harness.click(harness.center(close));
    harness.frame(Vec::new());

    assert!(
        text_within(harness.document(), root, "Nothing open").is_some(),
        "closing the last paper leaves a tab saying there is nothing open"
    );
    let reopen = text_within(harness.document(), root, "Splits").expect("the files list names it");
    harness.click(harness.center(reopen));
    harness.frame(Vec::new());

    assert!(
        text_within(harness.document(), root, "Nothing open").is_none(),
        "opening a paper takes the place the empty workspace held"
    );
    assert!(
        text_within(harness.document(), root, "Splits").is_some(),
        "the paper it opened is the one the files list named"
    );
}
