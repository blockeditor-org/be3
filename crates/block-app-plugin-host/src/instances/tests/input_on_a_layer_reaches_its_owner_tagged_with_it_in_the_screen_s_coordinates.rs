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
fn input_on_a_layer_reaches_its_owner_tagged_with_it_in_the_screen_s_coordinates() {
    let mut instances = layered(pos2(10.0, 10.0));
    let menu = Rect::from_min_size(pos2(150.0, 50.0), vec2(60.0, 40.0));
    report_layers(
        &mut instances,
        vec![layer(3, block_plugin_api::LayerParent::Region, menu)],
    );
    let input = beui::ForwardedInput {
        events: vec![press(pos2(160.0, 60.0))],
        rect: window(),
        hovered: true,
        focused: false,
        pointer: Some(pos2(160.0, 60.0)),
        modifiers: beui::Modifiers::NONE,
    };

    let messages = instances.forward_layer(INSTANCE, REGION, LayerId(3), &input);

    let events: Vec<_> = messages
        .into_iter()
        .filter_map(|message| match message {
            Outbound::Plugin(PluginCall::Input(batch)) => Some(batch.events),
            _ => None,
        })
        .flatten()
        .filter(|event| matches!(event.input, Input::PointerButton { .. }))
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].layer, Some(LayerId(3)));
    assert!(
        matches!(events[0].input, Input::PointerButton { pos, pressed: true, .. } if pos == pos2(150.0, 50.0)),
        "the press is in the region's own points, not the window's: {:?}",
        events[0].input
    );
    assert!(
        instances
            .forward_layer(INSTANCE, REGION, LayerId(4), &input)
            .is_empty(),
        "input for a layer the region does not have open goes nowhere"
    );
}
