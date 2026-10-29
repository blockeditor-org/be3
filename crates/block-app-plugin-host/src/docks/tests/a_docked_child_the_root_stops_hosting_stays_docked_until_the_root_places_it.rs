use super::*;

fn placed_outer() -> [DockedChild; 1] {
    [DockedChild {
        child: OUTER_CHILD,
        tab: 100,
        block: OUTER_BLOCK,
    }]
}

#[test]
fn a_docked_child_the_root_stops_hosting_stays_docked_until_the_root_places_it() {
    let mut docks = nested();
    docks.placed(ROOT, &[], None);
    host(&mut docks, DockHold::Hosted(hosted_by_root()));

    docks.heard("outer", vec![declare(OUTER, None)]);
    host(&mut docks, DockHold::Placing);
    docks.placed(ROOT, &[], None);
    docks.placed(ROOT, &[], None);
    docks.placed(ROOT, &[], None);
    assert!(
        docks.docked(OUTER),
        "a child that withdrew its dock stays docked while the root lays out its tab again"
    );
    docks.placed(ROOT, &placed_outer(), None);
    docks.placed(ROOT, &placed_outer(), None);
    docks.placed(ROOT, &placed_outer(), None);
    assert!(docks.docked(OUTER), "placed in its tab, it stays docked");
    host(&mut docks, DockHold::Released);
    assert!(
        docks.docked(OUTER),
        "and a placed child stays docked when its tab body lets go of its hold"
    );

    docks.heard("outer", vec![declare(OUTER, Some(outer()))]);
    docks.placed(ROOT, &[], None);
    host(&mut docks, DockHold::Hosted(hosted_by_root()));
    docks.heard("outer", vec![declare(OUTER, None)]);
    host(&mut docks, DockHold::Placing);
    docks.offscreen(ROOT);
    assert!(
        docks.docked(OUTER),
        "a root that is not on screen leaves its children as they were"
    );
    docks.heard("outer", vec![declare(OUTER, Some(outer()))]);
    assert!(docks.docked(OUTER));
    docks.placed(ROOT, &placed_outer(), None);
    assert!(
        docks.docked(OUTER),
        "and it is docked when the root shows again"
    );
}
