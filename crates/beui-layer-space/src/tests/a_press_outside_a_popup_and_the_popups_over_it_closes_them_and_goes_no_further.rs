use super::*;

fn open_as(
    space: &mut Space,
    now: Instant,
    placement: WindowPlacement,
    role: WindowRole,
) -> WindowId {
    let window = WindowId::next();
    reopen(space, now, window, placement, role);
    window
}

fn reopen(
    space: &mut Space,
    now: Instant,
    window: WindowId,
    placement: WindowPlacement,
    role: WindowRole,
) {
    space.request(&WindowRequest::Open {
        window,
        placement,
        size: SMALL,
        role,
    });
    send(space, now, []);
}

fn press(space: &mut Space, now: Instant, window: WindowId, at: Pos2) -> Vec<Incoming> {
    let pos = screen(space, space.panel(window).unwrap().world(at));
    send(
        space,
        now,
        [
            Input::PointerMoved(pos),
            button(pos, true),
            button(pos, false),
        ],
    )
}

fn closed(incoming: &[Incoming]) -> Vec<WindowId> {
    reports(incoming)
        .into_iter()
        .filter_map(|report| match report {
            WindowReport::Closed(window) => Some(window),
            _ => None,
        })
        .collect()
}

fn buttons(incoming: &[Incoming]) -> Vec<WindowId> {
    delivered(incoming)
        .into_iter()
        .filter(|(_, input)| matches!(input, Input::PointerButton { .. }))
        .map(|(window, _)| window)
        .collect()
}

#[test]
fn a_press_outside_a_popup_and_the_popups_over_it_closes_them_and_goes_no_further() {
    let (mut space, now) = space();
    let over = |window| WindowPlacement::Over {
        window,
        at: pos2(150.0, 120.0),
    };
    let popup = open_as(&mut space, now, over(WindowId::MAIN), WindowRole::Popup);
    let inner = open_as(&mut space, now, over(popup), WindowRole::Popup);

    let got = press(&mut space, now, inner, pos2(60.0, 60.0));
    assert!(
        closed(&got).is_empty(),
        "a press in a popup's own popup closes nothing"
    );
    assert_eq!(buttons(&got), [inner, inner], "and reaches it");
    assert_eq!(
        space.focused(),
        Some(WindowId::MAIN),
        "popups never take the focus"
    );

    let got = press(&mut space, now, popup, pos2(20.0, 20.0));
    assert_eq!(
        closed(&got),
        [inner],
        "a press in the popup closes only the popup over it"
    );
    assert!(
        buttons(&got).is_empty(),
        "and goes no further, as a press outside a menu does"
    );
    assert!(space.panel(inner).is_none());

    let inner = open_as(&mut space, now, over(popup), WindowRole::Popup);
    let got = press(&mut space, now, WindowId::MAIN, pos2(600.0, 500.0));
    assert_eq!(
        closed(&got),
        [popup, inner],
        "a press elsewhere closes them all"
    );
    assert!(
        buttons(&got).is_empty(),
        "and goes no further, its release neither"
    );
    assert!(space.panel(popup).is_none() && space.panel(inner).is_none());

    let got = press(&mut space, now, WindowId::MAIN, pos2(600.0, 500.0));
    assert_eq!(
        buttons(&got),
        [WindowId::MAIN, WindowId::MAIN],
        "the next press reaches the app"
    );

    let window = open_as(&mut space, now, over(WindowId::MAIN), WindowRole::Normal);
    let got = press(&mut space, now, WindowId::MAIN, pos2(600.0, 500.0));
    assert!(closed(&got).is_empty(), "a normal window stays");
    reopen(
        &mut space,
        now,
        window,
        over(WindowId::MAIN),
        WindowRole::Popup,
    );
    let got = press(&mut space, now, WindowId::MAIN, pos2(600.0, 500.0));
    assert_eq!(
        closed(&got),
        [window],
        "a window opened again as a popup is one"
    );
}
