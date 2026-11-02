use super::*;
use crate::focus::{Under, display_point, display_target};

#[test]
fn a_display_click_goes_to_what_is_under_it_and_not_to_block() {
    let display = entry(2, EntryKind::Display, Rect::new(1920, 0, 3840, 1080));
    assert_eq!(
        display_point(&display, Some(Point::new(10, 20))),
        Point::new(1930, 20)
    );
    assert_eq!(
        display_point(&display, Some(Point::new(5000, -4))),
        Point::new(3839, 0)
    );
    assert_eq!(display_point(&display, None), Point::new(2880, 540));
    assert_eq!(
        display_target(Under::Window(Handle(40))),
        Ok(Some(Handle(40)))
    );
    assert_eq!(display_target(Under::Nothing), Ok(None));
    assert_eq!(display_target(Under::Own), Err(FocusError::OnBlock));
}
