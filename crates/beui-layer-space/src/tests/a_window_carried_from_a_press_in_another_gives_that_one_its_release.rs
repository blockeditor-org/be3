use super::*;

#[test]
fn a_window_carried_from_a_press_in_another_gives_that_one_its_release() {
    let (mut space, now) = space();
    let start = centre() + vec2(-150.0, -100.0);
    let pressed = send(
        &mut space,
        now,
        [Input::PointerMoved(start), button(start, true)],
    );
    assert!(
        delivered(&pressed)
            .iter()
            .any(|(to, input)| *to == WindowId::MAIN
                && matches!(input, Input::PointerButton { pressed: true, .. })),
        "the press reaches the main window"
    );
    let (window, _) = open(&mut space, now, WindowPlacement::AtPointer);
    space.request(&WindowRequest::BeginMove(window));
    assert_eq!(
        space.mouse.carry.as_ref().map(|carry| carry.window),
        Some(window),
        "the main window's drag carries the window it opened"
    );

    let moved = start + vec2(100.0, 50.0);
    send(&mut space, now, [Input::PointerMoved(moved)]);
    let released = send(&mut space, now, [button(moved, false)]);
    assert_eq!(space.mouse.carry, None, "letting go ends the carry");
    let got = delivered(&released);
    let [
        (
            to,
            Input::PointerButton {
                pressed: false,
                ..
            },
        ),
    ] = got.as_slice()
    else {
        panic!("the release reaches the app once, not {got:?}");
    };
    assert_eq!(
        *to,
        WindowId::MAIN,
        "in the window that holds the press, so its drag ends"
    );
    assert_eq!(space.mouse.captured, None);

    let on_window = screen(
        &space,
        space
            .panel(window)
            .unwrap()
            .world(pos2(SMALL.x / 2.0, SMALL.y / 2.0)),
    );
    let clicked = send(
        &mut space,
        now,
        [
            Input::PointerMoved(on_window),
            button(on_window, true),
            button(on_window, false),
        ],
    );
    assert!(
        delivered(&clicked)
            .iter()
            .filter(|(_, input)| matches!(input, Input::PointerButton { .. }))
            .all(|(to, _)| *to == window),
        "a click afterwards reaches the carried window: {clicked:?}"
    );
}
