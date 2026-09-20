#![allow(dead_code)]

use super::*;

include!("../../../examples/dock.rs");

#[test]
fn the_dock_demo_opens_a_paper_from_the_files_it_lists() {
    let mut harness = Harness::sized(DockDemo::new().document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the demo built a root");
    assert_eq!(
        showing(harness.document(), root, "Notes"),
        1,
        "a paper that is not open is only named by the files list"
    );
    assert_eq!(
        showing(harness.document(), root, "Welcome"),
        3,
        "the paper the demo opens with is named by the list, its tab and its panel"
    );

    let row = text_within(harness.document(), root, "Notes").expect("the files list names it");
    harness.click(harness.center(row));
    harness.frame(Vec::new());

    assert_eq!(
        showing(harness.document(), root, "Notes"),
        3,
        "opening a paper gives it a tab and a panel beside the list that named it"
    );
    assert_eq!(
        showing(harness.document(), root, "Welcome"),
        2,
        "the paper it was opened over keeps its tab and stops being laid out"
    );
}

fn showing(document: &Document, id: NodeId, text: &str) -> usize {
    let laid_out = document.node_rect(id).is_some();
    let here =
        usize::from(laid_out && document.node_kind(id) == "text" && document.text(id) == text);
    here + document
        .children(id)
        .into_iter()
        .map(|child| showing(document, child, text))
        .sum::<usize>()
}
