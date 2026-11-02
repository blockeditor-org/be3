use super::*;

fn press(pos: Pos2) -> beui::Forwarded {
    beui::Forwarded::Pointer {
        event: beui::PointerEvent {
            pointer: beui::PointerId::Mouse,
            window: beui::WindowId::MAIN,
            phase: beui::PointerPhase::Down(beui::PointerButton::Primary),
            pos,
            modifiers: beui::Modifiers::NONE,
            clicks: 1,
            force: None,
            pen: None,
            predicted: None,
            time: std::time::Instant::now(),
            source: 0,
        },
        over: None,
    }
}

#[test]
fn input_on_an_anchored_view_reaches_its_own_screen_in_its_own_points() {
    let mut instances = layered(pos2(10.0, 10.0));
    let menu = Rect::from_min_size(pos2(150.0, 50.0), vec2(60.0, 40.0));
    report_anchored(&mut instances, vec![layer(3, 0, menu)]);
    let input = beui::ForwardedInput {
        events: vec![press(pos2(160.0, 60.0))],
        rect: window(),
        hovered: true,
        focused: false,
        pointer: Some(pos2(160.0, 60.0)),
        modifiers: beui::Modifiers::NONE,
    };

    let messages = instances.forward_anchored(INSTANCE, REGION, ViewId(3), &input);

    let events: Vec<_> = messages
        .into_iter()
        .filter_map(|message| match message {
            Outbound::Plugin(PluginCall::Input(batch)) => {
                assert_eq!(
                    batch.screen,
                    ScreenId::opened(3),
                    "input goes to the view's own screen"
                );
                Some(batch.events)
            }
            _ => None,
        })
        .flatten()
        .filter(|event| matches!(event.input, Input::PointerButton { .. }))
        .collect();
    assert_eq!(events.len(), 1);
    assert!(
        matches!(events[0].input, Input::PointerButton { pos, pressed: true, .. } if pos == pos2(10.0, 10.0)),
        "the press is in the anchored view's own points: {:?}",
        events[0].input
    );
    assert!(
        instances
            .forward_anchored(INSTANCE, REGION, ViewId(4), &input)
            .is_empty(),
        "input for a layer the region does not have open goes nowhere"
    );
}
