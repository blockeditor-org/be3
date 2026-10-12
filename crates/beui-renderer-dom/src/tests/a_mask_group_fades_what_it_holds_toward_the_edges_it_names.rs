use super::*;

use beui_core::display::Group;
use beui_core::fade::Fade;
use beui_core::geometry::{Rect, pos2};

#[test]
fn a_mask_group_fades_what_it_holds_toward_the_edges_it_names() {
    let viewport = Rect::from_min_max(pos2(20.0, 10.0), pos2(220.0, 310.0));
    let (outer, inner) = style::group(Group::Mask(Fade::new(viewport, [0.0, 0.0, 0.0, 24.0])), 1.0);
    assert_eq!(
        outer,
        "left:20px;width:200px;overflow-x:clip;top:10px;height:300px;overflow-y:clip;\
         mask-image:linear-gradient(to bottom,#000 0px,#000 276px,transparent 300px);\
         -webkit-mask-image:linear-gradient(to bottom,#000 0px,#000 276px,transparent 300px);"
    );
    assert_eq!(inner, "left:-20px;top:-10px;");

    let (outer, _) = style::group(
        Group::Mask(Fade::new(viewport, [8.0, 24.0, 0.0, 24.0])),
        1.0,
    );
    assert!(
        outer.contains("linear-gradient(to right,transparent 0px,#000 8px,#000 100%)"),
        "{outer}"
    );
    assert!(
        outer.contains(
            "linear-gradient(to bottom,transparent 0px,#000 24px,#000 276px,transparent 300px)"
        ),
        "{outer}"
    );
    assert!(outer.contains("mask-composite:intersect;"), "{outer}");
}
