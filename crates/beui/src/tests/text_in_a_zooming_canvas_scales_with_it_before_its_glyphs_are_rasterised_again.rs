use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Quad, quads};

fn glyph_span(output: &FrameOutput) -> [f32; 4] {
    quads(output.main(), 1.0)
        .list
        .iter()
        .filter_map(|quad| match quad {
            Quad::Glyph { rect, .. } => Some(*rect),
            _ => None,
        })
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |span, rect| {
            [
                span[0].min(rect[0]),
                span[1].min(rect[1]),
                span[2].max(rect[2]),
                span[3].max(rect[3]),
            ]
        })
}

fn near(span: [f32; 4], expected: [f32; 4], what: &str) {
    assert!(
        span.iter()
            .zip(expected)
            .all(|(at, want)| (at - want).abs() <= 2.0),
        "{what}: the text's glyphs span {span:?}, not about {expected:?}"
    );
}

fn zoomed(span: [f32; 4], scale: f32) -> [f32; 4] {
    span.map(|value| value * scale)
}

#[test]
fn text_in_a_zooming_canvas_scales_with_it_before_its_glyphs_are_rasterised_again() {
    let _root = crate::reactive::RootScope::new();
    let view = create_signal(Some(CanvasView::new(pos2(0.0, 0.0), 1.0)));
    let document = build_node(move || {
        view! {
            <Canvas view={view}>
                <CanvasItem x=40.0 y=40.0 width=200.0 height=40.0 scales=true>
                    <Text>"Zooming text"</Text>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let crisp = glyph_span(&harness.frame(Vec::new()));
    assert!(
        crisp[2] - crisp[0] > 40.0,
        "the text paints glyphs: {crisp:?}"
    );

    for scale in [2.0, 1.0, 0.5] {
        view.set(Some(CanvasView::new(pos2(0.0, 0.0), scale)));
        let moving = glyph_span(&harness.frame(Vec::new()));
        near(
            moving,
            zoomed(crisp, scale),
            "while the zoom moves, text scales with the canvas",
        );
        harness.advance(Duration::from_millis(200));
        let settled = glyph_span(&harness.frame(Vec::new()));
        near(
            settled,
            zoomed(crisp, scale),
            "once the zoom settles, text keeps the size it was zoomed to",
        );
    }
}
