use super::*;

use beui_core::fade::Fade;
use beui_core::geometry::{Rect, pos2, vec2};
use beui_core::painter::Entry;

#[test]
fn a_child_display_is_placed_inside_the_clip_it_entered() {
    let entry = Entry {
        translation: vec2(30.0, -120.0),
        clip: Rect::from_min_max(pos2(20.0, 10.0), pos2(220.0, 310.0)),
        shift: None,
        fade: Fade::NONE,
    };
    let (frame, content) = style::placement(entry, 1.0);
    assert_eq!(
        frame,
        "left:20px;width:200px;overflow-x:clip;top:10px;height:300px;overflow-y:clip;"
    );
    assert_eq!(content, "left:10px;top:-130px;");

    let (frame, content) = style::placement(
        Entry {
            translation: vec2(5.0, 6.0),
            ..Entry::NONE
        },
        1.0,
    );
    assert_eq!(frame, "");
    assert_eq!(content, "left:5px;top:6px;");
}
