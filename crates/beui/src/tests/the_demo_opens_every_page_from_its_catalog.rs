#![allow(dead_code)]

use super::*;

include!("../../examples/demo.rs");

#[test]
fn the_demo_opens_every_page_from_its_catalog() {
    let mut harness = Harness::sized(DemoApp::new().document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the demo built a root");
    for page in PAGES {
        open_demo_page(&mut harness, page.title());
        assert!(
            showing(harness.document(), root, page.title()) >= 2,
            "opening {} gives it a tab beside the catalog that named it",
            page.title()
        );
    }
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
