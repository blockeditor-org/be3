use beui::{Rect, Screen};

use super::*;

#[test]
fn it_shows_on_every_screen() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(640.0, 480.0));
    let right = Rect::from_min_size(pos2(640.0, 0.0), Vec2::new(460.0, 320.0));
    let document = build(move || {
        view! {
            <Root>
                <LockScreen
                    open=true
                    time="09:41"
                    date="Thursday, 8 October"
                    user="Ada Lovelace"
                    actions={vec![suspend()]}
                />
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(1100.0, 480.0));
    test.set_screens(vec![
        Screen::new("left", "Left", left),
        Screen::new("right", "Right", right),
    ]);
    assert!(left.contains(test.rect_of("lock.0").center()));
    assert!(right.contains(test.rect_of("lock.1").center()));
    assert!(test.shows("lock.0.password"));
    assert!(test.shows("lock.1.action.0"));
}
