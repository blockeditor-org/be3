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
    let close = closer(harness.document(), root, "Welcome").expect("the paper can be closed");

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

fn closer(document: &Document, root: NodeId, title: &str) -> Option<NodeId> {
    let label = text_within(document, root, title)?;
    let bar = document.node_rect(label)?;
    crosses(document, root)
        .into_iter()
        .find(|(_, rect)| rect.left() > bar.right() && rect.top() < bar.bottom())
        .map(|(node, _)| node)
}

fn crosses(document: &Document, root: NodeId) -> Vec<(NodeId, Rect)> {
    let mut found = Vec::new();
    collect_crosses(document, root, &mut found);
    found
}

fn collect_crosses(document: &Document, id: NodeId, out: &mut Vec<(NodeId, Rect)>) {
    if document.node_kind(id) == "text"
        && document.text(id) == beui::icons::ICON_CLOSE
        && let Some(rect) = document.node_rect(id)
    {
        out.push((id, rect));
    }
    for child in document.children(id) {
        collect_crosses(document, child, out);
    }
}
