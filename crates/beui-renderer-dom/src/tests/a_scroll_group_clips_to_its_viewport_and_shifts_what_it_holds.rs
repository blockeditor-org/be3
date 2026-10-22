use super::*;

use beui_core::display::Group;
use beui_core::document::Document;
use beui_core::geometry::{Rect, pos2, vec2};
use beui_core::node::SpaceId;

#[test]
fn a_scroll_group_clips_to_its_viewport_and_shifts_what_it_holds() {
    let host = Document::new().create_canvas().id();
    let viewport = Rect::from_min_max(pos2(20.0, 10.0), pos2(220.0, 310.0));
    let scroll = Group::Scroll {
        space: SpaceId::inside(host, 1),
        viewport,
        offset: vec2(0.0, -50.0),
        scale: 1.0,
        raster: 1.0,
    };
    let (outer, inner) = style::group(scroll, 1.0);
    assert_eq!(
        outer,
        "left:20px;width:200px;overflow-x:clip;top:10px;height:300px;overflow-y:clip;"
    );
    assert_eq!(inner, "left:-20px;top:-60px;");
}
