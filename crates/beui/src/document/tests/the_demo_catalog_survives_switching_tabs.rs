#![allow(dead_code)]

use super::*;

include!("../../../examples/demo.rs");

const TABS: [&str; 7] = ["List", "Strip", "Load", "Name", "Choices", "Menus", "Tree"];

#[test]
fn the_demo_catalog_survives_switching_tabs() {
    for width in [360.0, 700.0, 760.0, 1100.0] {
        for label in TABS {
            switch_to(Vec2::new(width, 700.0), label);
        }
    }
}

fn switch_to(viewport: Vec2, label: &str) {
    let mut harness = Harness::sized(DemoApp::new().document, viewport);
    harness.frame(Vec::new());
    let root = harness.document.root().expect("the demo built a root");
    let wanted = format!("\"{label}\"");
    let Some(tab) = labelled(harness.document(), root, &wanted) else {
        return;
    };
    let Some(rect) = harness.document.node_rect(tab) else {
        return;
    };
    harness.click(rect.center());
    harness.frame(Vec::new());
}

fn labelled(document: &Document, id: NodeId, detail: &str) -> Option<NodeId> {
    if document.node_detail(id).as_deref() == Some(detail) {
        return Some(id);
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| labelled(document, child, detail))
}
