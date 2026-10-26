use super::*;

#[test]
fn a_carried_panel_follows_the_cursor_and_the_head() {
    let (mut space, now) = space();
    let grab = centre() + vec2(0.0, -100.0);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grab), button(grab, true)],
    );
    let grabbed = main(&space)
        .hit(space.head.ray(PLATFORM, grab.to_vec2()))
        .unwrap();
    space.begin_carry(Pointer::Mouse, WindowId::MAIN);
    let distance = space.mouse.carry.as_ref().unwrap().distance;

    let facing = |space: &Space, at: Pos2| {
        let panel = main(space);
        let hit = panel.hit(space.head.ray(PLATFORM, at.to_vec2())).unwrap();
        let toward = space.head.position - hit.point;
        let normal = panel.normal();
        normal.x.atan2(normal.z) - toward.x.atan2(toward.z)
    };
    let before = facing(&space, grab);

    let moved = centre() + vec2(150.0, -50.0);
    assert_eq!(
        delivered(&send(&mut space, now, [Input::PointerMoved(moved)])),
        [],
        "the app sees no moves while its window is carried"
    );
    let under = main(&space)
        .hit(space.head.ray(PLATFORM, moved.to_vec2()))
        .expect("the panel moved under the cursor");
    assert!(
        (under.pos - grabbed.pos).length() < 0.5,
        "the grabbed point stays under the cursor: {:?} then {:?}",
        grabbed.pos,
        under.pos
    );
    assert!(
        (under.distance - distance).abs() < 1e-3,
        "at the same distance"
    );
    let normal = main(&space).normal();
    assert!(normal.x < -0.1, "swung right, it turns: {normal:?}");
    assert!(
        (facing(&space, moved) - before).abs() < 1e-3,
        "turning about the grabbed point to face the viewer as before: {before} then {}",
        facing(&space, moved)
    );

    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(400.0, 0.0))]);
    let turned = main(&space)
        .hit(space.head.ray(PLATFORM, moved.to_vec2()))
        .expect("the panel turned with the head");
    assert!((turned.pos - grabbed.pos).length() < 0.5);
    send(&mut space, now, steer(false));

    let released = send(&mut space, now, [button(moved, false)]);
    assert_eq!(space.mouse.carry, None, "letting go ends the carry");
    let got = delivered(&released);
    let [
        (
            window,
            Input::PointerButton {
                pos,
                pressed: false,
                ..
            },
        ),
    ] = got.as_slice()
    else {
        panic!("the app gets the release, not {got:?}");
    };
    assert_eq!(*window, WindowId::MAIN);
    assert!(
        (*pos - grabbed.pos).length() < 0.5,
        "where it grabbed: {pos:?}"
    );
    let reports = placed(&released);
    let [WindowReport::Placed { window, placement }] = reports.as_slice() else {
        panic!("the new place is reported, not {reports:?}");
    };
    assert_eq!(*window, WindowId::MAIN);
    assert_eq!(*placement, main(&space).placement());
}
