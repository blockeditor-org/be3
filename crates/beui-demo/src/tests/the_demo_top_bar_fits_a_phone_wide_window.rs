use super::*;

const PHONE: Vec2 = Vec2::new(360.0, 700.0);

/// On a phone-wide window the top bar keeps every control inside the window: its actions as
/// icons, and its theme choice under More rather than as a select too wide for the window.
#[test]
fn the_demo_top_bar_fits_a_phone_wide_window() {
    let mut test = demo(PHONE);
    test.frame(Vec::new());
    assert!(
        !test.shown("demo.theme"),
        "no select on a phone-wide window"
    );
    for id in [
        "demo.toolbar",
        "demo.bar.pin.demo.inspector",
        "demo.bar.pin.demo.reset",
        "demo.bar.menu",
    ] {
        let rect = test.rect_of(id);
        assert!(
            rect.left() >= 0.0 && rect.right() <= PHONE.x,
            "{id} at {rect:?} spills off a {PHONE:?} window"
        );
    }
    test.snapshot("the_demo_top_bar_on_a_phone");

    test.click("demo.bar.menu");
    test.frame(Vec::new());
    assert!(
        test.shown("demo.bar.menu.demo.theme"),
        "the theme choice is under More"
    );
}
