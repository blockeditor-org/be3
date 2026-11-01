use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::Direction;

#[test]
fn a_scroll_shorter_than_its_view_stretches_what_fills_it() {
    let _root = crate::reactive::RootScope::new();
    let filler = NodeRef::new();
    let last = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal>
                    <List direction=Direction::Horizontal spacing=0.0>
                        <Frame width=40.0 />
                        <Frame @node_ref=&filler @sizing=ItemSize::Percent(100.0) />
                        <Frame @node_ref=&last width=40.0 />
                    </List>
                </Offset>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(last.get()).max.x,
        VIEWPORT.x,
        "content shorter than the view is laid out as long as the view, as a browser's is"
    );
    assert_eq!(harness.rect(filler.get()).width(), VIEWPORT.x - 80.0);
}
