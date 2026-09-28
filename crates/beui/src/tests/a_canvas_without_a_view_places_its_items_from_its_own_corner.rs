use super::*;
use crate::reactive::{Canvas, CanvasItem, build, view, with_document};

#[test]
fn a_canvas_without_a_view_places_its_items_from_its_own_corner() {
    let item = Rc::new(Cell::new(None));
    let sink = item.clone();

    let document = build(move || {
        let canvas = view! {
            <Canvas>
                <CanvasItem x=12.0 y=8.0 width=30.0 height=15.0 />
            </Canvas>
        };
        sink.set(
            with_document(|document| document.children(canvas))
                .first()
                .copied(),
        );
        canvas
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let item = item.get().expect("the canvas item was created");
    assert_eq!(
        harness.rect(item),
        Rect::from_min_size(pos2(12.0, 8.0), Vec2::new(30.0, 15.0))
    );
}
