use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_canvas_places_its_view_from_its_own_corner() {
    let _root = crate::reactive::RootScope::new();
    let (first, lower) = (NodeRef::new(), NodeRef::new());
    let view = Some(CanvasView::new(pos2(10.0, 5.0), 2.0));

    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Canvas view={view} width=200.0 height=100.0>
                    <CanvasItem x=3.0 y=4.0 width=20.0 height=10.0>
                        <Frame @node_ref=&first width=40.0 height=20.0 />
                    </CanvasItem>
                </Canvas>
                <Canvas view={view} width=200.0 height=100.0>
                    <CanvasItem x=3.0 y=4.0 width=20.0 height=10.0>
                        <Frame @node_ref=&lower width=40.0 height=20.0 />
                    </CanvasItem>
                </Canvas>
            </List>
        }
    });

    let mut harness = Harness::sized(document, vec2(400.0, 400.0));
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(first.get()).min,
        pos2(16.0, 13.0),
        "a view's origin is in the canvas's own points"
    );
    assert_eq!(
        harness.rect(lower.get()).min,
        pos2(16.0, 113.0),
        "so the same view in a canvas laid out 100 points down places its items 100 points down"
    );
}
