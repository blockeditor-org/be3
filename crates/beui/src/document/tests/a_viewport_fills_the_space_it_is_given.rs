use super::*;
use crate::reactive::{Frame, NodeRef, Viewport, build, view};

#[test]
fn a_viewport_fills_the_space_it_is_given() {
    let viewport = NodeRef::new();
    let node = viewport.clone();
    let document = build(move || {
        view! {
            <Frame width=120.0 height=80.0>
                <Viewport @node_ref={&node} drawing={None} />
            </Frame>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(viewport.get()).size(),
        Vec2::new(120.0, 80.0),
        "a viewport takes the whole box it was laid out in"
    );
}
