use super::*;

use beui_core::display::Group;
use beui_core::geometry::{Rect, pos2, vec2};

#[test]
fn a_clip_group_cuts_its_content_to_the_rectangle_it_names() {
    let clip = Rect::from_min_max(pos2(20.0, 10.0), pos2(220.0, 310.0));
    let (outer, inner) = style::group(Group::Clip(clip), 1.0);
    assert_eq!(
        outer,
        "left:20px;width:200px;overflow-x:clip;top:10px;height:300px;overflow-y:clip;"
    );
    assert_eq!(inner, "left:-20px;top:-10px;");
    assert_eq!(style::placement(vec2(5.0, 6.0), 1.0), "left:5px;top:6px;");
}
