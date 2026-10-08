use super::*;

#[test]
fn a_fullscreen_window_covers_only_its_own_screen() {
    let mut harness = Harness::new();
    let left = Rect::from_min_size(Pos2::ZERO, Vec2::new(SCREEN.x / 2.0, SCREEN.y));
    let right = Rect::from_min_max(pos2(SCREEN.x / 2.0, 0.0), pos2(SCREEN.x, SCREEN.y));
    harness.app.set_screens(vec![left, right]);
    let (window, id) = harness.open();
    let under = harness.rect("test.under").center().y;

    window.toplevel.as_ref().unwrap().set_fullscreen(None);
    harness.acknowledge(&window);
    assert_eq!(
        harness.window(id),
        left,
        "the window covers the screen it was shown on"
    );
    assert_eq!(
        harness.client.received.size,
        Some((left.width() as i32, left.height() as i32))
    );

    harness.frame(vec![Event::PointerMoved(pos2(right.center().x, under))]);
    harness.settle();
    assert!(
        harness.under.borrow().hovered,
        "the ui on the other screen still hears the pointer"
    );

    harness.frame(vec![Event::PointerMoved(pos2(left.center().x, under))]);
    harness.settle();
    assert!(
        !harness.under.borrow().hovered,
        "the ui on the covered screen does not"
    );
}
