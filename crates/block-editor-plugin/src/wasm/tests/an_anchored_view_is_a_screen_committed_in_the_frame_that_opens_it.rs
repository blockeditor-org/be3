use super::*;

#[test]
fn an_anchored_view_is_a_screen_committed_in_the_frame_that_opens_it() {
    let mut rig = Rig::new(Some(space()));
    let menu = Rect::from_min_size(pos2(220.0, 130.0), vec2(60.0, 40.5));
    let submenu = Rect::from_min_size(pos2(280.0, 140.0), vec2(50.0, 30.0));
    rig.open(vec![layer(4, 0, menu), layer(5, 4, submenu)]);
    let commit = rig.draw();

    assert_eq!(
        rig.shared.seen.borrow().spaces,
        [Some(space())],
        "the frame was laid out against the layer space its screen was requested with"
    );
    let anchored = Rig::anchored(&commit);
    let views: Vec<ViewId> = anchored
        .iter()
        .map(|(screen, _)| rig.placed(*screen).view)
        .collect();
    assert_eq!(
        views,
        [ViewId(4), ViewId(5)],
        "both views are reported in the frame that opened them, each as a screen of its own"
    );
    assert!(
        anchored.iter().all(|(screen, _)| screen.is_opened()),
        "numbered by the plugin: {anchored:?}"
    );
    assert_eq!(anchored[0].1.rect, menu);
    assert_eq!(
        anchored[1].1.parent,
        ViewId(4),
        "the submenu hangs from the menu"
    );
    let surfaces: Vec<u32> = anchored
        .iter()
        .map(|(screen, _)| rig.placed(*screen).surface)
        .collect();
    assert!(
        surfaces[0] != surfaces[1] && !surfaces.contains(&rig.screen_surface()),
        "each view has a surface of its own: {surfaces:?}"
    );
    assert!(
        rig.surfaces.take_relaid(),
        "the runtime sends the host the layout with the views before the commit"
    );
    let menu_report = commit
        .reports
        .iter()
        .find(|report| report.screen == anchored[0].0)
        .expect("the menu is reported");
    assert_eq!(
        menu_report.painted,
        [Rect::from_min_size(pos2(0.0, 0.0), menu.size())],
        "in its own points"
    );
    assert_eq!(
        rig.shared.seen.borrow().painted,
        [(ViewId(4), [60, 81]), (ViewId(5), [50, 60])],
        "each view was painted at the raster over its zoom on each axis, in the frame that opened it"
    );
    let damage = commit.presented.expect("the frame presented").damage;
    assert!(
        damage.iter().any(|damage| damage.screen == anchored[0].0),
        "the frame presented the views with the screen: {damage:?}"
    );
}
