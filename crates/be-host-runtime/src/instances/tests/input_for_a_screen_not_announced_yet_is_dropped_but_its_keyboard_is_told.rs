use super::*;

#[test]
fn input_for_a_screen_not_announced_yet_is_dropped_but_its_keyboard_is_told() {
    let mut instances = placed();
    let pressed = beui::Forwarded::Pointer {
        event: beui::PointerEvent {
            pointer: beui::PointerId::Mouse,
            window: beui::WindowId::MAIN,
            phase: beui::PointerPhase::Down(beui::PointerButton::Primary),
            pos: Pos2::new(10.0, 10.0),
            modifiers: beui::Modifiers::NONE,
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
    let typed = |text: &str| beui::Forwarded::Input(beui::Event::Text(text.to_owned()));
    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(
            vec![beui::Forwarded::Focus(true), pressed, typed("a")],
            None,
            true,
        ),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert!(
        instances.gate(messages).is_empty(),
        "a screen the app does not have yet hears nothing"
    );

    let screens = instances.next_screens(PASS).screens;
    let announced = instances.announce(screens);
    assert_eq!(
        input_events(&announced),
        [Input::Focus(true)],
        "once announced the screen hears it has the keyboard, and not the press or keys it missed"
    );

    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![typed("b")], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert_eq!(
        input_events(&instances.gate(messages)),
        [Input::Text("b".to_owned())]
    );
}
