use super::*;
use crate::native::with_document;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_canvas_lays_out_only_the_items_the_view_can_see() {
    let _root = crate::reactive::RootScope::new();
    let items = create_signal(Vec::new());

    let document = build_node(move || {
        let canvas = view! {
            <Canvas view={Some(CanvasView::new(Pos2::ZERO, 1.0))}>
                <CanvasItem x=10.0 y=10.0 width=20.0 height=20.0 />
                <CanvasItem x=5000.0 y=10.0 width=20.0 height=20.0 />
            </Canvas>
        };
        items.set(with_document(|document| document.children(canvas)));
        canvas
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let items = items.get_untracked();
    assert!(harness.document().node_rect(items[0]).is_some());
    assert!(harness.document().node_rect(items[1]).is_none());
}
