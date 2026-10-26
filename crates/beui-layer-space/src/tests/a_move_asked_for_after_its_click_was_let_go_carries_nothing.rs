use super::*;

#[test]
fn a_move_asked_for_after_its_click_was_let_go_carries_nothing() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.5);
    let middle = pos2(SMALL.x / 2.0, SMALL.y / 2.0);
    let bar = screen(&space, space.panel(window).unwrap().world(middle));
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(bar),
            button(bar, true),
            button(bar, false),
        ],
    );
    space.request(&WindowRequest::BeginMove(window));
    assert_eq!(
        space.mouse.carry, None,
        "a plugin's move request that arrives after the click it answers starts no carry"
    );

    let pose = space.panel(window).unwrap().pose;
    let elsewhere = pos2(120.0, 120.0);
    let ray = space.head.ray(PLATFORM, elsewhere.to_vec2());
    assert_eq!(space.pick(ray).map(|hit| hit.window), Some(WindowId::MAIN));
    let clicked = send(
        &mut space,
        now,
        [
            Input::PointerMoved(elsewhere),
            button(elsewhere, true),
            button(elsewhere, false),
        ],
    );
    assert_eq!(
        space.panel(window).unwrap().pose,
        pose,
        "moving and clicking elsewhere leaves the window where it was"
    );
    let buttons: Vec<_> = delivered(&clicked)
        .into_iter()
        .filter(|(_, input)| matches!(input, Input::PointerButton { .. }))
        .map(|(to, _)| to)
        .collect();
    assert_eq!(
        buttons,
        [WindowId::MAIN, WindowId::MAIN],
        "and the click reaches the window under the cursor"
    );
    assert!(placed(&clicked).is_empty(), "no carry ends: {clicked:?}");
}
