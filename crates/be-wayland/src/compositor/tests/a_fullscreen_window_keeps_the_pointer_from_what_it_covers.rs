use super::*;

fn hover_and_scroll(harness: &mut Harness, at: Pos2) {
    harness.frame(vec![Event::PointerMoved(at)]);
    harness.frame(vec![
        Event::PointerMoved(at),
        Event::Scroll(Vec2::new(0.0, -30.0)),
    ]);
    harness.settle();
}

#[test]
fn a_fullscreen_window_keeps_the_pointer_from_what_it_covers() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let (window, _id) = harness.open();
    let under = harness.rect("test.under").center();

    hover_and_scroll(&mut harness, under);
    assert!(
        harness.under.borrow().hovered,
        "uncovered, the ui hears the pointer"
    );
    assert_ne!(harness.under.borrow().scrolled, Vec2::ZERO);
    harness.frame(vec![Event::PointerMoved(outside())]);
    *harness.under.borrow_mut() = Under::default();
    harness.client.received.pointer_entered = None;

    window.toplevel.as_ref().unwrap().set_fullscreen(None);
    harness.acknowledge(&window);
    harness
        .client
        .attach_unsent(&window, SCREEN.x as i32, SCREEN.y as i32);
    harness.settle();
    hover_and_scroll(&mut harness, under);
    harness.frame(vec![
        Event::PointerButton {
            pos: under,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: beui::Modifiers::NONE,
        },
        Event::PointerButton {
            pos: under,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: beui::Modifiers::NONE,
        },
    ]);
    harness.settle();

    assert!(
        !harness.under.borrow().hovered,
        "the ui under a fullscreen window is not hovered"
    );
    assert_eq!(
        harness.under.borrow().scrolled,
        Vec2::ZERO,
        "the wheel over a fullscreen window does not scroll what it covers"
    );
    assert!(
        harness.client.received.pointer_entered.is_some(),
        "the fullscreen window gets the pointer"
    );
    assert_ne!(harness.client.received.scrolled, 0.0, "and the wheel");
    assert!(
        harness.client.received.buttons.contains(&(0x110, true)),
        "and the press"
    );
}
