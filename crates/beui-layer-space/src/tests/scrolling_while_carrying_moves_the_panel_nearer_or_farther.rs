use super::*;

#[test]
fn scrolling_while_carrying_moves_the_panel_nearer_or_farther() {
    let (mut space, now) = space();
    send(
        &mut space,
        now,
        [Input::PointerMoved(centre()), button(centre(), true)],
    );
    space.begin_carry(Pointer::Mouse, WindowId::MAIN);
    let start = space.mouse.carry.as_ref().unwrap().distance;
    let ray = space.head.ray(PLATFORM, centre().to_vec2());

    let scrolled = send(
        &mut space,
        now,
        [Input::ScrollStarted, scroll(3.0), Input::ScrollEnded],
    );
    assert_eq!(
        delivered(&scrolled),
        [],
        "the app gets no scroll during a carry"
    );
    let farther = space.mouse.carry.as_ref().unwrap().distance;
    assert!(farther > start, "scrolling forward pushes it away");
    let hit = main(&space).hit(ray).unwrap();
    assert!(
        (hit.distance - farther).abs() < 1e-3,
        "the panel moved along the ray"
    );

    send(&mut space, now, [scroll(-6.0)]);
    assert!(
        space.mouse.carry.as_ref().unwrap().distance < start,
        "scrolling back pulls it in"
    );
    send(&mut space, now, [scroll(-1000.0)]);
    assert_eq!(
        space.mouse.carry.as_ref().unwrap().distance,
        crate::carry::NEAREST,
        "but not into the viewer's face"
    );

    send(&mut space, now, [button(centre(), false)]);
    assert_eq!(
        delivered(&send(&mut space, now, [scroll(1.0)])),
        [(WindowId::MAIN, scroll(1.0))],
        "afterwards scrolling reaches the app again"
    );
}
