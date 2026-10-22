use super::*;

use beui_core::color::Color32;
use beui_core::font::{FontId, Galley, GalleyLine};
use beui_core::geometry::{Rect, Rotation, pos2, vec2};
use beui_core::painter::Shape;

#[test]
fn scaled_text_is_set_in_a_font_as_large_as_it_is_drawn() {
    let galley = Galley::new(
        "Zoom",
        FontId::proportional(12.0),
        vec2(30.0, 16.0),
        16.0,
        12.0,
        Vec::new(),
        vec![GalleyLine {
            top: 0.0,
            range: 0..4,
            cursors: vec![(0, 0.0), (4, 30.0)],
        }],
        1.0,
    );
    let text = Shape::Text {
        origin: pos2(10.0, 20.0),
        galley,
        scale: 2.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        rotation: Rotation::NONE,
        clip: Rect::EVERYTHING,
    };
    let Some(style::Look::Text { css, lines }) = style::look(&text, 1.0, None) else {
        panic!("text is a text look");
    };
    assert!(
        css.starts_with("left:10px;top:20px;width:60px;height:32px;"),
        "{css}"
    );
    assert!(
        css.contains(" 24px "),
        "the font is set at twice its size: {css}"
    );
    assert_eq!(
        lines[0].css,
        "left:0px;top:0px;height:32px;line-height:32px;"
    );
}
