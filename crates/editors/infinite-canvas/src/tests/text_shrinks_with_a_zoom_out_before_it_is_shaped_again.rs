use super::*;
use block_editor_beui::beui::{Rect, native::Quad, native::Shape, native::quads, pos2};

const WRITTEN: &str = "Zooming out shrinks this note";

fn text_width(editor: &BeuiTest<CanvasApp>) -> f32 {
    let output = editor.drawn().expect("the canvas has drawn");
    let mut shapes = Vec::new();
    for layer in output.layers.iter() {
        layer.flatten(&mut shapes);
    }
    let laid = shapes
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Text {
                origin,
                galley,
                scale,
                ..
            } if galley.text().contains(WRITTEN) => {
                Some(Rect::from_min_size(origin, galley.size() * scale).expand(4.0))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("the note is drawn"));
    let (left, right) = quads(output, 1.0)
        .list
        .iter()
        .filter_map(|quad| match quad {
            Quad::Glyph { rect, .. }
                if laid.contains(pos2(rect[0], rect[1]))
                    && laid.contains(pos2(rect[2], rect[3])) =>
            {
                Some((rect[0], rect[2]))
            }
            _ => None,
        })
        .fold((f32::MAX, f32::MIN), |(left, right), (min, max)| {
            (left.min(min), right.max(max))
        });
    right - left
}

#[test]
fn text_shrinks_with_a_zoom_out_before_it_is_shaped_again() {
    let mut note = shaped(Uuid::from_u128(1), CanvasShape::Text);
    note.transform = CanvasTransform::new(
        CanvasPoint::new(0.0, 0.0),
        CanvasPoint::new(320.0, 60.0),
        0.0,
    );
    note.text = WRITTEN.into();
    let mut editor = editor(&[note]);
    editor.run();
    editor.advance(std::time::Duration::from_millis(500));
    let crisp = text_width(&editor);
    assert!(crisp > 100.0, "the note paints its text, {crisp} wide");

    editor.click("infinite-canvas.pill.zoom-out");
    editor.run();
    let moving = text_width(&editor);
    editor.snapshot("text_shrinks_with_a_zoom_out_before_it_is_shaped_again");
    editor.advance(std::time::Duration::from_millis(500));
    let settled = text_width(&editor);
    assert!(
        settled < crisp * 0.9,
        "zooming out shrinks the text from {crisp} to {settled} wide"
    );
    assert!(
        (moving - settled).abs() <= 3.0,
        "in the frame a zoom out lands in, the text is {moving} wide, not the {settled} \
         it is once the zoom settles"
    );
}
