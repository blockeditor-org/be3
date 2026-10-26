use super::*;
use crate::painter::Shape;
use crate::primitives::*;
use crate::reactive::*;

fn tallest_glyph(output: &FrameOutput) -> u32 {
    output
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text { galley, .. } => Some(galley.clone()),
            _ => None,
        })
        .flat_map(|galley| {
            galley
                .glyphs()
                .iter()
                .filter_map(|glyph| glyph.image().map(|image| image.height))
                .collect::<Vec<_>>()
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn zooming_a_canvas_paints_none_of_its_scaled_items_until_their_text_settles() {
    let _root = crate::reactive::RootScope::new();
    let view = create_signal(Some(CanvasView::new(pos2(0.0, 0.0), 1.0)));
    let items = create_signal(Vec::new());
    let document = build_node(move || {
        let canvas = view! {
            <Canvas view={view}>
                <CanvasItem x=10.0 y=10.0 width=80.0 height=20.0 scales=true>
                    <Text>"first"</Text>
                </CanvasItem>
                <CanvasItem x=10.0 y=40.0 width=80.0 height=20.0 scales=true>
                    <Frame fill=Color32::WHITE radius=0.0>
                        <Text>"second"</Text>
                    </Frame>
                </CanvasItem>
            </Canvas>
        };
        items.set(with_document(|document| document.children(canvas)));
        canvas
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let first = harness.frame(Vec::new());
    let shapes = first.main().shapes().len();
    let crisp = tallest_glyph(&first);

    view.set(Some(CanvasView::new(pos2(25.0, 15.0), 2.0)));
    let output = harness.frame(Vec::new());
    let work = harness.document().performance().latest.work;
    assert_eq!(
        work.placed, 1,
        "zooming scales the canvas's items without laying any of them out again"
    );
    assert_eq!(
        work.painted_nodes, 1,
        "zooming scales the canvas's items without painting any of them again"
    );
    assert_eq!(output.main().shapes().len(), shapes);
    let framed = Rect::from_min_size(pos2(45.0, 95.0), Vec2::new(160.0, 40.0));
    let moved = output
        .main()
        .shapes()
        .iter()
        .any(|shape| matches!(shape, Shape::Rect { rect, .. } if *rect == framed));
    assert!(moved, "the framed item is painted where the zoom put it");
    let items = items.get_untracked();
    assert_eq!(harness.rect(items[1]), framed);
    assert_eq!(
        tallest_glyph(&output),
        crisp,
        "while the zoom moves, text keeps the raster it had"
    );

    harness.advance(Duration::from_millis(200));
    let settled = harness.frame(Vec::new());
    let work = harness.document().performance().latest.work;
    assert!(
        work.painted_nodes > 1,
        "once the zoom settles, the scaled items paint their text again"
    );
    assert_eq!(
        (work.measured, work.placed),
        (0, 0),
        "and rasterising it again lays nothing out"
    );
    assert!(
        tallest_glyph(&settled) >= crisp * 2 - 1,
        "text is rasterised again for the scale it is shown at"
    );
    assert!(
        settled
            .main()
            .shapes()
            .iter()
            .any(|shape| matches!(shape, Shape::Rect { rect, .. } if *rect == framed))
    );
}
