use super::*;

#[test]
fn the_room_beside_an_anchor_is_what_an_overlay_shrinks_to() {
    let anchor = rect(150.0, 120.0, 250.0, 150.0);
    assert_eq!(room(VIEWPORT, anchor, Placement::BELOW_START), Vec2::new(400.0, 150.0));
    assert_eq!(
        room(VIEWPORT, anchor, Placement::ABOVE_CENTER.gap(4).margin(8)),
        Vec2::new(384.0, 138.0),
        "the roomier side less the gap, and the margin off every edge"
    );
    assert_eq!(room(VIEWPORT, anchor, Placement::RIGHT_START), VIEWPORT.size());
    assert_eq!(
        room(VIEWPORT, rect(150.0, 0.0, 250.0, 300.0), Placement::BELOW_START),
        VIEWPORT.size(),
        "an anchor leaving no room either side is covered, with the window's room"
    );
    assert_eq!(room(VIEWPORT, anchor, Placement::Menu), VIEWPORT.size());
    for side in SIDES {
        for align in ALIGNS {
            let placement = placement(side, align).gap(4).margin(8);
            let shown = placed(placement, anchor, 1000.0, 1000.0);
            assert_eq!(
                shown.size(),
                room(VIEWPORT, anchor, placement),
                "{side:?} {align:?} too large for the window shrinks to its room"
            );
            assert!(!side.vertical() || !shown.intersects(anchor));
        }
    }
}
