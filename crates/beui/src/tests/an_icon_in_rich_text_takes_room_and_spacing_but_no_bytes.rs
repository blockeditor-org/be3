use super::*;
use crate::icons::ICON_STAR;
use crate::reactive::*;
use beui_core::font::{FontId, Galley, TextLayout};
use beui_core::rich::{Piece, RichOptions};

const SPACING: f32 = 5.0;

#[test]
fn an_icon_in_rich_text_takes_room_and_spacing_but_no_bytes() {
    let context = Context::new(crate::FreetypeFonts::default());
    let mut shaper =
        |text: &str, font: FontId| -> Galley { context.layout(text, font, TextLayout::DEFAULT) };
    let style = SpanStyle::new(FontId::proportional(14.0), Color32::WHITE);
    let icon_style = SpanStyle::new(FontId::icons(18.0), Color32::WHITE);
    let icon = |glyph| Piece::Icon {
        glyph,
        style: icon_style,
        spacing: SPACING,
        text_size: 14.0,
    };
    let options = RichOptions {
        wrap_width: f32::INFINITY,
        padding: (0.0, 0.0),
        style,
        hang: 0,
        anywhere: false,
        least: false,
    };
    let width = shaper(ICON_STAR.as_str(), FontId::icons(18.0)).size().x;
    let plain = RichLayout::new(&[Piece::text("Save", style)], options, &mut shaper);
    let iconed = RichLayout::new(
        &[icon(ICON_STAR.as_str()), Piece::text("Save", style)],
        options,
        &mut shaper,
    );
    let alone = RichLayout::new(&[icon(ICON_STAR.as_str())], options, &mut shaper);
    let empty = RichLayout::new(
        &[icon(""), Piece::text("Save", style)],
        options,
        &mut shaper,
    );

    assert_eq!(iconed.lines[0].range, 0..4, "an icon holds no bytes");
    assert_eq!(
        iconed.caret_rect(0, 1.0).min.x,
        width + SPACING,
        "the text after an icon starts past it and its spacing"
    );
    assert_eq!(iconed.size.x, plain.size.x + width + SPACING);
    assert_eq!(
        alone.size.x, width,
        "an icon with nothing beside it keeps no spacing"
    );
    assert_eq!(
        iconed.size.y, plain.size.y,
        "an icon as big as its text scales to does not make the line taller"
    );
    assert_eq!(
        empty.size.x, plain.size.x,
        "an icon with no glyph takes no room"
    );
}
