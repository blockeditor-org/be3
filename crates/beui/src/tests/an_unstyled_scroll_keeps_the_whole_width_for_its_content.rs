use super::*;
use crate::reactive::{ItemSize, List, NodeRef, build, view};

const CONTENT_HEIGHT: f32 = 2000.0;

#[test]
fn an_unstyled_scroll_keeps_the_whole_width_for_its_content() {
    let scroll = NodeRef::new();
    let held = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <unstyled::Scroll @node_ref=&held @sizing=ItemSize::Percent(100.0)>
                    <Frame height=CONTENT_HEIGHT />
                </unstyled::Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let children = harness.document().children(scroll.get());
    assert_eq!(children.len(), 1, "an unstyled scroll painted a scrollbar");
    assert_eq!(harness.rect(children[0]), harness.rect(scroll.get()));
}
