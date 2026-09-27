use super::*;
use crate::painter::Shape;
use crate::reactive::{
    Canvas, CanvasItem, CanvasView, Text, build, create_signal, view, with_reactive_scope,
};

#[test]
fn panning_a_canvas_lays_out_and_paints_none_of_its_items_again() {
    let (view, set_view) = create_signal(Some(CanvasView::new(pos2(0.0, 0.0), 1.0)));
    let document = build(move || {
        view! {
            <Canvas view={view}>
                <CanvasItem x=10.0 y=10.0 width=80.0 height=20.0>
                    <Text string="first" />
                </CanvasItem>
                <CanvasItem x=10.0 y=40.0 width=80.0 height=20.0>
                    <Frame color=Color32::WHITE radius=0>
                        <Text string="second" />
                    </Frame>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let first = harness.frame(Vec::new()).shapes().len();

    with_reactive_scope(harness.document_mut(), move || {
        set_view.set(Some(CanvasView::new(pos2(25.0, 15.0), 1.0)))
    });
    let output = harness.frame(Vec::new());
    let work = harness.document().performance().latest.work;
    assert_eq!(
        work.placed, 1,
        "panning moves the canvas's items without laying any of them out again"
    );
    assert_eq!(
        work.painted_nodes, 1,
        "panning moves the canvas's items without painting any of them again"
    );
    assert_eq!(output.shapes().len(), first);
    let moved = output.shapes().iter().any(|shape| {
        matches!(shape, Shape::Rect { rect, .. } if rect.min == pos2(35.0, 55.0))
    });
    assert!(moved, "the framed item is painted where the pan moved it");
}
