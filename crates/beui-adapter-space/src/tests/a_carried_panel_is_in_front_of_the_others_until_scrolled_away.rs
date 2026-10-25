use super::*;

#[test]
fn a_carried_panel_is_in_front_of_the_others_until_scrolled_away() {
    let (mut space, now) = space();
    let behind = open_ahead(&mut space, now, 2.0);
    let ray = space.head.ray(PLATFORM, centre().to_vec2());
    let hit = |space: &Space| space.pick(ray).map(|hit| hit.window);
    assert_eq!(
        hit(&space),
        Some(WindowId::MAIN),
        "the main panel hides the one behind it"
    );

    send(&mut space, now, [Input::PointerMoved(centre())]);
    space.request(&WindowRequest::BeginMove(behind));
    assert_eq!(space.lifted(), Some(behind));
    assert_eq!(
        hit(&space),
        Some(behind),
        "carried, it is in front of every other panel though it is farther"
    );
    assert!(
        space
            .cursor_point()
            .is_some_and(|point| (point.distance(space.head.position) - 2.0).abs() < 0.01),
        "and the cursor is on it"
    );

    send(&mut space, now, [scroll(-1.0)]);
    assert_eq!(space.lifted(), Some(behind), "pulled nearer, it stays in front");
    send(&mut space, now, [scroll(2.0)]);
    assert_eq!(
        space.lifted(),
        None,
        "pushed farther than where it was picked up, it takes its place by depth"
    );
    assert_eq!(hit(&space), Some(WindowId::MAIN));

    send(&mut space, now, [scroll(-1.0)]);
    assert_eq!(space.lifted(), Some(behind), "pulled back, it is in front again");

    send(&mut space, now, [button(centre(), true), button(centre(), false)]);
    assert_eq!(space.lifted(), None, "once let go, depth decides again");
    assert_eq!(hit(&space), Some(WindowId::MAIN));
}
