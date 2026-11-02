use super::*;

#[test]
fn input_for_a_screen_the_plugin_does_not_have_yet_reaches_it_once_announced() {
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
        },
        over: None,
    };
    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![beui::Forwarded::Focus(true), pressed], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert!(
        instances.gate(messages).is_empty(),
        "a screen the plugin does not have yet hears nothing yet"
    );
    assert!(instances.take_announced_input().is_empty());

    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    let held = instances.take_announced_input();
    let heard = input_events(&held);
    assert_eq!(
        heard.first(),
        Some(&Input::Focus(true)),
        "once the screen is announced it hears the focus it missed first, got {heard:?}"
    );
    assert!(
        heard
            .iter()
            .any(|input| matches!(input, Input::PointerButton { pressed: true, .. })),
        "and then the press, rather than losing it, got {heard:?}"
    );

    let typed = beui::Forwarded::Input(beui::Event::Text("a".to_owned()));
    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![typed], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert_eq!(
        input_events(&instances.gate(messages)),
        [Input::Text("a".to_owned())]
    );
}
