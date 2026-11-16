use super::*;
use be_sdk_api::Modifiers;

fn pressed(pos: Pos2, modifiers: beui::Modifiers) -> beui::Forwarded {
    let beui::Forwarded::Pointer { event, over } = moved(pos) else {
        unreachable!("moved makes a pointer event");
    };
    beui::Forwarded::Pointer {
        event: beui::PointerEvent {
            phase: beui::PointerPhase::Down(beui::PointerButton::Primary),
            modifiers,
            ..event
        },
        over,
    }
}

#[test]
fn modifiers_let_go_in_one_view_are_held_again_in_another() {
    let mut instances = placed();
    let window = ViewId(9);
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    instances.report(
        INSTANCE,
        View::window(window),
        Uuid::nil(),
        role,
        &Arc::new(crate::AppCatalog::default()),
        Some(be_sdk_api::FrameSpec::default()),
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    instances.place(INSTANCE, window, Placement { rect, clip: rect });
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    let at = pos2(50.0, 50.0);
    let logo = beui::Modifiers::LOGO;
    let press_in_window = |instances: &mut Instances| {
        let input = forwarded(vec![pressed(at, logo)], Some(at), false);
        let (messages, _) =
            instances.forward(INSTANCE, window, &input, &|_, _| beui::Aim::Own, &|_| None);
        input_events(&messages)
    };
    let held = Input::Modifiers(Modifiers {
        logo: true,
        ..Modifiers::default()
    });

    assert_eq!(
        press_in_window(&mut instances).first(),
        Some(&held),
        "a press with Super held tells the app Super is down"
    );
    assert_eq!(
        input_events(&instances.intercepted(INSTANCE, REGION, None, beui::Modifiers::NONE)),
        [Input::Modifiers(Modifiers::default())],
        "letting go of Super reaches the app through its main view"
    );
    assert_eq!(
        press_in_window(&mut instances).first(),
        Some(&held),
        "the app keeps one set of modifiers, so the window's next Super press says Super is \
         down again even though the window last sent the same"
    );

    for focused in [true, false] {
        let focus = forwarded(vec![beui::Forwarded::Focus(focused)], None, false);
        instances.forward(INSTANCE, window, &focus, &|_, _| beui::Aim::Own, &|_| None);
    }
    assert_eq!(
        press_in_window(&mut instances).first(),
        Some(&held),
        "an app that lost the keyboard let go of its modifiers, so the next press holds them again"
    );
}
