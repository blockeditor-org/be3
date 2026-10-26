use super::*;

#[test]
fn moving_a_window_carries_its_panel() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.5);
    let main_pose = main(&space).pose;
    let grab = centre() + vec2(20.0, 10.0);
    let grabbed = on(&space, window, grab);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grab), button(grab, true)],
    );
    space.request(&WindowRequest::BeginMove(window));
    assert_eq!(space.carry.as_ref().map(|carry| carry.window), Some(window));

    let moved = grab + vec2(-200.0, 60.0);
    assert_eq!(
        delivered(&send(&mut space, now, [Input::PointerMoved(moved)])),
        []
    );
    assert!(
        (on(&space, window, moved) - grabbed).length() < 0.5,
        "the grabbed point follows the cursor"
    );
    assert_eq!(main(&space).pose, main_pose, "the main window stays");

    let released = send(&mut space, now, [button(moved, false)]);
    assert_eq!(space.carry, None);
    assert_eq!(
        delivered(&released)
            .iter()
            .map(|(to, _)| *to)
            .collect::<Vec<_>>(),
        [window]
    );
    let reports = placed(&released);
    let [
        WindowReport::Placed {
            window: placed,
            placement,
        },
    ] = reports.as_slice()
    else {
        panic!("the new place is reported, not {reports:?}");
    };
    assert_eq!(*placed, window);
    assert_eq!(*placement, space.panel(window).unwrap().placement());
}
