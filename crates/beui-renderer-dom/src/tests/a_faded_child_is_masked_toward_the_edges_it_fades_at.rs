use super::*;

use beui_core::fade::Fade;
use beui_core::geometry::{Rect, pos2, vec2};
use beui_core::painter::Entry;

#[test]
fn a_faded_child_is_masked_toward_the_edges_it_fades_at() {
    let viewport = Rect::from_min_max(pos2(20.0, 10.0), pos2(220.0, 310.0));
    let entry = Entry {
        translation: vec2(20.0, -40.0),
        clip: viewport,
        shift: Some(vec2(0.0, -50.0)),
        fade: Fade::new(viewport, [0.0, 0.0, 0.0, 24.0]),
    };
    let (frame, _) = style::placement(entry, 1.0);
    assert_eq!(
        frame,
        "left:20px;width:200px;overflow-x:clip;top:10px;height:300px;overflow-y:clip;\
         mask-image:linear-gradient(to bottom,#000 0px,#000 276px,transparent 300px);\
         -webkit-mask-image:linear-gradient(to bottom,#000 0px,#000 276px,transparent 300px);"
    );

    let (frame, _) = style::placement(
        Entry {
            fade: Fade::new(viewport, [8.0, 24.0, 0.0, 24.0]),
            ..entry
        },
        1.0,
    );
    assert!(
        frame.contains("linear-gradient(to right,transparent 0px,#000 8px,#000 100%)"),
        "{frame}"
    );
    assert!(
        frame.contains(
            "linear-gradient(to bottom,transparent 0px,#000 24px,#000 276px,transparent 300px)"
        ),
        "{frame}"
    );
    assert!(frame.contains("mask-composite:intersect;"), "{frame}");

    let (frame, _) = style::placement(
        Entry {
            fade: Fade::NONE,
            ..entry
        },
        1.0,
    );
    assert!(!frame.contains("mask"), "{frame}");
}
