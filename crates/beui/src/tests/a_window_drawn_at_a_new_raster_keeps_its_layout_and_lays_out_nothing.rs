use super::*;
use crate::primitives::{Text, Window, WindowHandle};
use crate::{WindowId, WindowReport};

#[derive(Debug, PartialEq)]
struct Laid {
    origin: Pos2,
    size: Vec2,
    lines: Vec<std::ops::Range<usize>>,
}

fn texts(shapes: &[Shape]) -> Vec<(Laid, f32)> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text { origin, galley, .. } => Some((
                Laid {
                    origin: *origin,
                    size: galley.size(),
                    lines: galley
                        .lines()
                        .iter()
                        .map(|line| line.range.clone())
                        .collect(),
                },
                galley.pixels_per_point(),
            )),
            _ => None,
        })
        .collect()
}

fn widest_glyph(shapes: &[Shape]) -> u32 {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text { galley, .. } => galley
                .glyphs()
                .iter()
                .filter_map(|glyph| glyph.image().map(|image| image.width))
                .max(),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn a_window_drawn_at_a_new_raster_keeps_its_layout_and_lays_out_nothing() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let document = build_node(move || {
        view! {
            <Window id={window} size={vec2(150.0, 120.0)}>|_: WindowHandle|
                <Text @test_id="words" color=Color32::WHITE wrap=Wrap::Words>
                    "Pinching a panel draws it sharper without moving a single word of it"
                </Text>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    let shown = output.window(window).expect("the window is shown");
    let before = texts(&shown.shapes());
    assert!(!before.is_empty(), "the window paints its text");
    let rect = harness.rect(harness.find("words"));
    let glyph = widest_glyph(&shown.shapes());

    for raster in [1.37, 2.0, 0.8] {
        harness.report_window(WindowReport::Rastered {
            window,
            pixels_per_point: raster,
        });
        let output = harness.frame(Vec::new());
        let latest = harness.document().performance().latest;
        assert_eq!(
            (latest.work.measured, latest.work.placed),
            (0, 0),
            "drawing a window at {raster} pixels per point lays nothing out again"
        );
        assert_eq!(harness.rect(harness.find("words")), rect);
        let shown = output.window(window).expect("the window is shown");
        let now = texts(&shown.shapes());
        assert_eq!(
            now.iter().map(|(laid, _)| laid).collect::<Vec<_>>(),
            before.iter().map(|(laid, _)| laid).collect::<Vec<_>>(),
            "at {raster} the text wraps and sits where it did"
        );
        assert!(
            now.iter().all(|(_, scale)| *scale == raster),
            "its glyphs are rasterised at {raster} pixels per point: {now:?}"
        );
        let wide = widest_glyph(&shown.shapes()) as f32;
        assert!(
            (wide - glyph as f32 * raster).abs() <= 2.0 + raster,
            "a glyph rasterised at {raster} is about {raster} times as wide: {wide} against {glyph}"
        );
    }

    harness.rescale(2.0);
    harness.frame(Vec::new());
    assert!(
        harness.document().performance().latest.work.placed > 0,
        "a change of the platform's scale lays the document out again"
    );
}
