use super::*;

#[test]
fn each_cursor_is_looked_up_by_its_css_name_then_its_x11_names() {
    assert_eq!(
        names(CursorIcon::Default),
        [
            "default",
            "left_ptr",
            "arrow",
            "top_left_arrow",
            "left_arrow"
        ]
    );
    assert_eq!(names(CursorIcon::Text), ["text", "xterm", "ibeam"]);
    assert_eq!(names(CursorIcon::PointingHand)[..2], ["pointer", "hand2"]);
    assert_eq!(
        names(CursorIcon::ResizeHorizontal)[..2],
        ["ew-resize", "sb_h_double_arrow"]
    );
    assert_eq!(
        names(CursorIcon::ResizeNorthWest),
        ["nw-resize", "top_left_corner"]
    );
    assert_eq!(names(CursorIcon::Grabbing)[..2], ["grabbing", "closedhand"]);
    assert!(
        names(CursorIcon::None).is_empty(),
        "no cursor is shown for None"
    );
    for icon in CursorIcon::ALL {
        if icon != CursorIcon::None {
            assert_eq!(
                names(icon)[0],
                icon.name(),
                "{icon:?} is first looked up by its CSS name"
            );
        }
    }
}
