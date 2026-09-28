use super::*;

use beui_core::color::Color32;
use beui_core::geometry::{Rect, Rotation, pos2};
use beui_core::painter::Shape;

#[test]
fn a_shape_partly_outside_its_clip_is_cut_with_an_inset() {
    let rect = |clip| Shape::Rect {
        rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 50.0)),
        corner_radius: 0.0,
        stroke_width: 0.0,
        color: Color32::from_rgba_unmultiplied(255, 0, 0, 255),
        rotation: Rotation::NONE,
        clip,
    };
    let shape = rect(Rect::from_min_max(pos2(10.0, 0.0), pos2(80.0, 40.0)));
    let Some(style::Look::Box(css)) = style::look(&shape, 1.0, false, None) else {
        panic!("a rect is a box");
    };
    assert!(
        css.contains("clip-path:inset(0px 20px 10px 10px);"),
        "{css}"
    );

    let hidden = rect(Rect::from_min_max(pos2(200.0, 0.0), pos2(300.0, 40.0)));
    assert_eq!(style::look(&hidden, 1.0, false, None), None);
}
