use super::*;

use beui_core::display::Group;
use beui_core::geometry::Vec2;

#[test]
fn a_scaled_group_scales_what_it_holds() {
    let scaled = Group::Transform {
        offset: Vec2::ZERO,
        scale: 2.0,
    };
    let (outer, inner) = style::group(scaled, 1.0);
    assert_eq!(outer, "");
    assert_eq!(inner, "left:0px;top:0px;transform:scale(2);");
}
