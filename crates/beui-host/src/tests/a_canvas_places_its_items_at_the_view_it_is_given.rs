use super::*;
use crate::native::with_document;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_canvas_places_its_items_at_the_view_it_is_given() {
    let _root = crate::reactive::RootScope::new();
    let items = create_signal(Vec::new());

    let document = build_node(move || {
        let canvas = view! {
            <Canvas view={Some(CanvasView::new(pos2(50.0, 20.0), 2.0))}>
                <CanvasItem x=0.0 y=0.0 width=10.0 height=10.0 />
                <CanvasItem x=30.0 y=40.0 width=20.0 height=5.0 />
            </Canvas>
        };
        items.set(with_document(|document| document.children(canvas)));
        canvas
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let items = items.get_untracked();
    assert_eq!(
        harness.rect(items[0]),
        Rect::from_min_size(pos2(50.0, 20.0), Vec2::new(20.0, 20.0))
    );
    assert_eq!(
        harness.rect(items[1]),
        Rect::from_min_size(pos2(110.0, 100.0), Vec2::new(40.0, 10.0))
    );
}
