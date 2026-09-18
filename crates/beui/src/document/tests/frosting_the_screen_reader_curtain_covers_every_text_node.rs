use super::*;
use crate::painter::Shape;

#[test]
fn frosting_the_screen_reader_curtain_covers_every_text_node() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.enable_screen_reader();

    let output = harness.frame(Vec::new());
    assert!(frosted(&harness, &output));

    harness.click(harness.screen_reader_control_center("frost"));
    let output = harness.frame(Vec::new());
    assert!(!frosted(&harness, &output));
}

fn frosted(harness: &Harness, output: &crate::FrameOutput) -> bool {
    let words = painted_words(harness.document().shapes());
    assert!(!words.is_empty(), "the document painted no text");
    words.iter().all(|words| {
        output.shapes().iter().any(|shape| {
            matches!(
                shape,
                Shape::Rect { rect, color, stroke_width, .. }
                    if *stroke_width == 0.0
                        && color.alpha() == 255
                        && rect.contains(words.min)
                        && rect.contains(words.max)
            )
        })
    })
}

fn painted_words(shapes: &[Shape]) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text {
                origin,
                galley,
                clip,
                ..
            } => Some(Rect::from_min_size(*origin, galley.size()).intersect(*clip)),
            _ => None,
        })
        .filter(Rect::is_positive)
        .collect()
}
