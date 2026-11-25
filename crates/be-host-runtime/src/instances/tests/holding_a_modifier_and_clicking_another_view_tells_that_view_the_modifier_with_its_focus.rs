use super::*;

#[test]
fn holding_a_modifier_and_clicking_another_view_tells_that_view_the_modifier_with_its_focus() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.announce(screens);
    let ctrl = beui::Modifiers {
        ctrl: true,
        ..beui::Modifiers::NONE
    };
    // The app heard Ctrl through some other view, which then keeps the keyboard no more.
    let mut held = forwarded(Vec::new(), None, false);
    held.modifiers = ctrl;
    let click = beui::Forwarded::Pointer {
        event: beui::PointerEvent {
            pointer: beui::PointerId::Mouse,
            window: beui::WindowId::MAIN,
            phase: beui::PointerPhase::Down(beui::PointerButton::Primary),
            pos: Pos2::new(10.0, 10.0),
            modifiers: ctrl,
            clicks: 1,
            force: None,
            pen: None,
            predicted: None,
            time: std::time::Instant::now(),
            source: 0,
            rested: false,
        },
        over: None,
    };
    held.events = vec![beui::Forwarded::Focus(true), click];
    held.focused = true;
    let (messages, _) =
        instances.forward(INSTANCE, REGION, &held, &|_, _| beui::Aim::Own, &|_| None);
    let events = input_events(&instances.gate(messages));
    assert_eq!(events.first(), Some(&Input::Focus(true)));
    assert_eq!(
        events.get(1),
        Some(&Input::Modifiers(ctrl)),
        "the view is told Ctrl is held right after it gets the keyboard: {events:?}"
    );
    assert!(
        matches!(
            events.get(2),
            Some(Input::PointerButton { pressed: true, .. })
        ),
        "then the click follows: {events:?}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Input::Modifiers(_)))
            .count(),
        1,
        "and it is not told again with the click: {events:?}"
    );
}
