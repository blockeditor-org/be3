use beui::{Rect, Screen, pos2};

use super::*;

#[test]
fn it_shows_on_every_screen() {
    let _root = RootScope::new();
    let left = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(640.0, 480.0));
    let right = Rect::from_min_size(pos2(640.0, 0.0), Vec2::new(460.0, 320.0));
    let open = create_signal(true);
    let outcome = create_signal("");
    let mut test = DocumentTest::new(prompt(open, outcome), Vec2::new(1100.0, 480.0));
    test.set_screens(vec![
        Screen::new("left", "Left", left),
        Screen::new("right", "Right", right),
    ]);
    assert!(left.contains(test.rect_of("keep-changes.0").center()));
    assert!(right.contains(test.rect_of("keep-changes.1").center()));
}
