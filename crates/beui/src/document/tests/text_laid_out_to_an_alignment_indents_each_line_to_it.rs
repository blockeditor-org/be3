use super::*;
use crate::base::TextAlign;
use crate::font::{FontId, TextLayout};

#[test]
fn text_laid_out_to_an_alignment_indents_each_line_to_it() {
    let context = Context::new();
    let font = FontId::proportional(14.0);
    let output = context.run(RawInput::default(), |context| {
        let painter = context.painter();
        for align in [TextAlign::Start, TextAlign::Center, TextAlign::End] {
            let galley = painter.layout_text(
                "wide line\ni",
                font,
                TextLayout {
                    align,
                    ..TextLayout::DEFAULT
                },
            );
            painter.galley(Pos2::ZERO, galley, Color32::WHITE);
        }
        let spaced = painter.layout_text(
            "one\ntwo",
            font,
            TextLayout {
                line_spacing: 2.0,
                ..TextLayout::DEFAULT
            },
        );
        painter.galley(Pos2::ZERO, spaced, Color32::WHITE);
    });
    let galleys: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            crate::Shape::Text { galley, .. } => Some(galley.clone()),
            _ => None,
        })
        .collect();

    let second = |galley: &crate::Galley| galley.line_rects(Pos2::ZERO)[1].left();
    assert_eq!(
        second(&galleys[0]),
        0.0,
        "a start-aligned line starts flush"
    );
    assert!(
        second(&galleys[1]) > second(&galleys[0]),
        "a centred short line is indented"
    );
    assert!(
        second(&galleys[2]) > second(&galleys[1]),
        "an end-aligned short line is indented further"
    );
    assert!(
        galleys[3].line_height() > galleys[0].line_height() * 1.9,
        "doubling the line spacing doubles the height of a line"
    );
}
