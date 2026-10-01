use super::*;

use beui_core::color::Color32;
use beui_core::geometry::{Rect, Rotation, pos2};
use beui_core::painter::Shape;

#[test]
fn a_stroked_rect_is_a_border_and_a_filled_one_a_background() {
    let rect = |stroke_width| Shape::Rect {
        rect: Rect::from_min_max(pos2(10.0, 20.0), pos2(40.0, 30.0)),
        corner_radius: 4.0.into(),
        stroke_width,
        color: Color32::from_rgba_unmultiplied(0, 128, 255, 255),
        rotation: Rotation::NONE,
        clip: Rect::EVERYTHING,
    };
    let filled = rect(0.0);
    let Some(style::Look::Box(css)) = style::look(&filled, 2.0, false, None) else {
        panic!("a rect is a box");
    };
    assert_eq!(
        css,
        "left:10px;top:20px;width:30px;height:10px;border-radius:4px;background:rgba(0,128,255,1);"
    );

    let stroked = rect(1.0);
    let Some(style::Look::Box(css)) = style::look(&stroked, 2.0, false, None) else {
        panic!("a rect is a box");
    };
    assert!(
        css.ends_with("border:1px solid rgba(0,128,255,1);"),
        "{css}"
    );
}
