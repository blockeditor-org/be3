use super::*;
use crate::painter::Shape;
use crate::primitives::*;
use crate::reactive::*;

fn wraps(output: &FrameOutput) -> Vec<(Vec<std::ops::Range<usize>>, Vec2)> {
    output
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text { galley, .. } => Some((
                galley
                    .lines()
                    .iter()
                    .map(|line| line.range.clone())
                    .collect(),
                galley.size(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn text_in_a_zoomed_canvas_wraps_where_it_did_at_every_zoom() {
    let _root = crate::reactive::RootScope::new();
    let view = create_signal(Some(CanvasView::new(pos2(0.0, 0.0), 1.0)));
    let document = build_node(move || {
        view! {
            <Canvas view={view}>
                <CanvasItem x=10.0 y=10.0 width=130.0 height=120.0 scales=true>
                    <Text>"Pinching a canvas zooms its words without moving where they wrap"</Text>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let laid = wraps(&harness.frame(Vec::new()));
    assert!(
        laid.len() > 1 || laid.iter().any(|(lines, _)| lines.len() > 1),
        "the text wraps: {laid:?}"
    );
    for scale in [1.37, 2.6, 0.7] {
        view.set(Some(CanvasView::new(pos2(0.0, 0.0), scale)));
        harness.frame(Vec::new());
        harness.advance(Duration::from_millis(200));
        let settled = harness.frame(Vec::new());
        assert_eq!(
            wraps(&settled),
            laid,
            "zoomed to {scale}, the text wraps where it did and keeps its size in points"
        );
        let rastered = settled.main().shapes().iter().any(|shape| {
            matches!(shape, Shape::Text { galley, .. } if galley.pixels_per_point() == scale)
        });
        assert!(rastered, "its glyphs are rasterised for {scale}");
    }
}
