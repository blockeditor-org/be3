use super::*;
use crate::{TextAlign, TextLayout};

#[test]
fn text_laid_out_in_a_drawing_measures_what_it_draws() {
    let context = Context::new(crate::native::FreetypeFonts::default());
    let font = FontId::proportional(14.0);
    let origin = pos2(10.0, 20.0);
    let mut measured = None;
    let output = context.run(|context| {
        let painter = wrap_painter(&context.painter());
        let block = painter.lay_out_text(
            "one two three four",
            font,
            TextLayout {
                align: TextAlign::Center,
                ..TextLayout::wrapped(60.0)
            },
        );
        painter.draw_text(origin, &block, Color32::WHITE);
        measured = Some(block);
    });
    let block = measured.unwrap();
    let galley = output.main().shapes().iter().find_map(|shape| match shape {
        crate::native::Shape::Text {
            origin: at, galley, ..
        } => Some((*at, galley.clone())),
        _ => None,
    });
    let (at, galley) = galley.expect("the text is drawn");

    let lines = block.lines();
    assert!(lines.len() > 1, "the text wraps at the given width");
    assert_eq!(at, origin, "the text is drawn from its top-left corner");
    assert_eq!(
        galley.size(),
        block.size(),
        "what is drawn is the size measured"
    );
    assert_eq!(block.size().y, block.line_height() * lines.len() as f32);
    assert_eq!(
        lines
            .iter()
            .map(|line| line.bytes.clone())
            .collect::<Vec<_>>(),
        galley
            .lines()
            .iter()
            .map(|line| line.range.clone())
            .collect::<Vec<_>>(),
        "the lines measured are the lines drawn"
    );
    for line in &lines {
        assert!(line.rect.width() <= 60.0, "each line fits the wrap width");
        assert!(line.rect.right() <= block.size().x + 0.5);
        assert!(line.rect.top() < line.baseline && line.baseline < line.rect.bottom());
        assert_eq!(line.baseline - line.rect.top(), block.baseline());
    }
    assert!(
        lines[0].rect.left() >= 0.0 && lines.iter().any(|line| line.rect.left() > 0.0),
        "centred lines are indented"
    );
    assert!(block.has_every_glyph());
}
