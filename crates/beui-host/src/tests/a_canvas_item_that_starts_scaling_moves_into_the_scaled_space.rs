use super::*;
use crate::native::with_document;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_canvas_item_that_starts_scaling_moves_into_the_scaled_space() {
    let _root = crate::reactive::RootScope::new();
    let scales = create_signal(false);
    let items = create_signal(Vec::new());
    let document = build_node(move || {
        let canvas = view! {
            <Canvas view={Some(CanvasView::new(pos2(10.0, 0.0), 2.0))}>
                <CanvasItem x=5.0 y=5.0 width=20.0 height=10.0 scales={scales}>
                    <Frame fill=Color32::WHITE />
                </CanvasItem>
            </Canvas>
        };
        items.set(with_document(|document| document.children(canvas)));
        canvas
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let item = items.get_untracked()[0];
    let placed = Rect::from_min_size(pos2(20.0, 10.0), Vec2::new(40.0, 20.0));
    assert_eq!(harness.rect(item), placed);

    scales.set(true);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(item), placed);
}
