use super::*;

use beui_core::color::Color32;
use beui_core::geometry::{Rect, pos2};
use beui_core::painter::Shape;

#[test]
fn a_top_shape_rises_above_the_rest_of_its_layer() {
    let line = Shape::Line {
        from: pos2(0.0, 0.0),
        to: pos2(10.0, 0.0),
        width: 2.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        clip: Rect::EVERYTHING,
    };
    let Some(style::Look::Box(main)) = style::look(&line, 1.0, false, None) else {
        panic!("a line is a box");
    };
    let Some(style::Look::Box(top)) = style::look(&line, 1.0, true, None) else {
        panic!("a line is a box");
    };
    assert!(!main.contains("z-index"));
    assert_eq!(top, format!("{main}z-index:1;"));
}
