use super::*;

fn focus_reports(incoming: &[Incoming]) -> Vec<(WindowId, bool)> {
    incoming
        .iter()
        .filter_map(|incoming| match incoming {
            Incoming::Window(WindowReport::Focused { window, focused }) => {
                Some((*window, *focused))
            }
            _ => None,
        })
        .collect()
}

fn keys(incoming: &[Incoming]) -> usize {
    delivered(incoming)
        .iter()
        .filter(|(_, input)| matches!(input, Input::Key(_)))
        .count()
}

#[test]
fn clicking_a_panel_or_its_chrome_focuses_it_until_another_or_nothing_is_clicked() {
    let (mut space, now) = space();
    let over = open_ahead(&mut space, now, 0.4);
    let lit = |space: &Space, window| space.chrome_opacity(window) == 1.0;
    assert_eq!(
        space.focused(),
        Some(WindowId::MAIN),
        "the app starts focused"
    );
    assert!(lit(&space, WindowId::MAIN) && !lit(&space, over));

    let inside = screen(&space, space.panel(over).unwrap().world(pos2(100.0, 75.0)));
    let sent = send(
        &mut space,
        now,
        [Input::PointerMoved(inside), button(inside, true)],
    );
    assert_eq!(
        focus_reports(&sent),
        [(WindowId::MAIN, false), (over, true)]
    );
    assert!(lit(&space, over) && !lit(&space, WindowId::MAIN));
    let sky = pos2(5.0, 5.0);
    send(&mut space, now, [Input::PointerMoved(sky)]);
    assert!(
        lit(&space, over),
        "a drag that leaves the panel keeps its chrome lit"
    );
    send(&mut space, now, [button(sky, false)]);
    let on_main = screen(&space, main(&space).world(pos2(60.0, 60.0)));
    send(&mut space, now, [Input::PointerMoved(on_main)]);
    assert!(
        lit(&space, over) && !lit(&space, WindowId::MAIN),
        "hovering another panel does not move the focus"
    );

    let grip = on_part(&space, WindowId::MAIN, Part::Scale(Corner::BottomRight));
    let sent = send(
        &mut space,
        now,
        [Input::PointerMoved(grip), button(grip, true)],
    );
    assert_eq!(
        focus_reports(&sent),
        [(over, false), (WindowId::MAIN, true)]
    );
    let beyond = grip + vec2(30.0, 20.0);
    send(&mut space, now, [Input::PointerMoved(beyond)]);
    assert!(
        lit(&space, WindowId::MAIN) && !lit(&space, over),
        "pressing chrome focuses its panel, and dragging it off keeps that"
    );
    send(&mut space, now, [button(beyond, false)]);
    assert_eq!(keys(&send(&mut space, now, [key(Key::A, 30, true)])), 1);
    assert_eq!(keys(&send(&mut space, now, [key(Key::A, 30, false)])), 1);

    let sent = send(
        &mut space,
        now,
        [
            Input::PointerMoved(sky),
            button(sky, true),
            button(sky, false),
        ],
    );
    assert_eq!(focus_reports(&sent), [(WindowId::MAIN, false)]);
    assert_eq!(space.focused(), None, "clicking nothing unfocuses");
    assert!(!lit(&space, WindowId::MAIN) && !lit(&space, over));
    assert_eq!(
        keys(&send(
            &mut space,
            now,
            [key(Key::A, 30, true), key(Key::A, 30, false)]
        )),
        0,
        "keys reach no window while none is focused"
    );

    send(
        &mut space,
        now,
        [
            Input::PointerMoved(on_main),
            button(on_main, true),
            button(on_main, false),
        ],
    );
    assert_eq!(space.focused(), Some(WindowId::MAIN));
    assert_eq!(keys(&send(&mut space, now, [key(Key::A, 30, true)])), 1);
}
