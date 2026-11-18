use super::*;

#[test]
fn every_cursor_is_found_by_its_css_name() {
    for icon in CursorIcon::ALL {
        assert_eq!(CursorIcon::from_name(icon.name()), Some(icon), "{icon:?}");
    }
    let mut names: Vec<_> = CursorIcon::ALL.iter().map(|icon| icon.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        CursorIcon::ALL.len(),
        "no two cursors share a name"
    );
    assert_eq!(
        CursorIcon::from_name("pointer"),
        Some(CursorIcon::PointingHand)
    );
    assert_eq!(
        CursorIcon::from_name("ew-resize"),
        Some(CursorIcon::ResizeHorizontal)
    );
    assert_eq!(CursorIcon::from_name("left_ptr"), None);
    assert_eq!(
        CursorIcon::from_name(CursorIcon::Touch.name()),
        Some(CursorIcon::Touch),
        "the touch cursor, which CSS has no keyword for, has a name of its own"
    );
    assert!(!names.contains(&CursorIcon::Touch.name()));
    assert_eq!(
        CursorIcon::Touch.css(),
        "crosshair",
        "in CSS the touch cursor falls back to the crosshair"
    );
    for icon in CursorIcon::ALL {
        assert_eq!(icon.css(), icon.name(), "{icon:?}'s name is its CSS value");
    }
}
