use super::*;

use std::rc::Rc;

use beui_core::color::Color32;
use beui_core::geometry::{Rect, Rotation, Vec2, pos2};
use beui_core::painter::Shape;
use beui_core::path::{LineJoin, Path, PathPaint, PathStyle, Stroke};

#[test]
fn a_path_is_an_svg_drawn_over_its_bounds() {
    let path = Path::new()
        .move_to(pos2(10.0, 10.0))
        .line_to(pos2(30.0, 10.0))
        .quad_to(pos2(30.0, 30.0), pos2(10.0, 30.0));
    let stroke = Stroke::new(2.0)
        .join(LineJoin::Round)
        .dashed([4.0, 2.0], 1.0);
    let shape = Shape::Path {
        path: Rc::new(PathPaint::new(path, PathStyle::Stroke(stroke))),
        scale: 1.0,
        offset: Vec2::new(5.0, 0.0),
        color: Color32::from_rgba_unmultiplied(0, 128, 255, 255),
        rotation: Rotation::NONE,
        clip: Rect::EVERYTHING,
    };
    let Some(style::Look::Box(css)) = style::look(&shape, 1.0, None) else {
        panic!("a path is a box");
    };

    assert!(
        css.starts_with("left:13px;top:8px;width:24px;height:24px;"),
        "{css}"
    );
    assert!(css.contains("d='M2 2L22 2Q22 22 2 22'"), "{css}");
    assert!(css.contains("stroke-linejoin='round'"), "{css}");
    assert!(
        css.contains("stroke-dasharray='4 2' stroke-dashoffset='1'"),
        "{css}"
    );
    assert!(!css.contains('#') && !css.contains('<'), "{css}");
}
