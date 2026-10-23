use super::*;
use crate::primitives::*;
use crate::reactive::*;

const CONTENT_HEIGHT: f32 = 2000.0;

#[test]
fn an_unstyled_scroll_keeps_the_whole_width_for_its_content() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <unstyled::Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=CONTENT_HEIGHT />
                </unstyled::Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let children = harness
        .document()
        .children(layers_of(harness.document(), scroll.get()));
    assert!(
        harness.document().children(children[1]).is_empty(),
        "an unstyled scroll built a scrollbar"
    );
    assert_eq!(harness.rect(children[0]), harness.rect(scroll.get()));
}
