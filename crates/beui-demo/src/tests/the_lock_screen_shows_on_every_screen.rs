use beui::{Rect, Screen};
use beui::icons::ICON_BEDTIME;
use beui::reactive::{Frame, build, view};
use beui::styled::{LockAction, LockScreen, use_theme};

use super::*;

#[test]
fn the_lock_screen_shows_on_every_screen() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(640.0, 480.0));
    let right = Rect::from_min_size(pos2(640.0, 0.0), Vec2::new(460.0, 320.0));
    let document = build(move || {
        let theme = use_theme();
        view! {
            <Frame color={theme.background.clone()}>
                <LockScreen
                    open=true
                    time="09:41"
                    date="Thursday, 8 October"
                    user="Ada Lovelace"
                    actions={vec![LockAction {
                        label: "Suspend".to_owned(),
                        glyph: ICON_BEDTIME.to_owned(),
                    }]}
                />
            </Frame>
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
    test.snapshot("lock_screen_on_two_screens");
}
