use super::*;

#[test]
fn a_beside_overlay_flips_at_each_window_edge() {
    let size = Vec2::new(60.0, 40.0);
    for align in ALIGNS {
        let at = |side: AnchorSide, anchor: Rect| {
            let shown = placed(placement(side, align), anchor, size.x, size.y);
            assert_eq!(
                shown.size(),
                size,
                "{side:?} {align:?} keeps its size when it flips"
            );
            assert!(
                !shown.intersects(anchor),
                "{side:?} {align:?} never covers its anchor"
            );
            shown
        };
        let near_bottom = rect(150.0, 270.0, 250.0, 290.0);
        assert_eq!(
            at(AnchorSide::Below, near_bottom).bottom(),
            270.0,
            "below flips above"
        );
        let near_top = rect(150.0, 10.0, 250.0, 30.0);
        assert_eq!(
            at(AnchorSide::Above, near_top).top(),
            30.0,
            "above flips below"
        );
        let near_right = rect(370.0, 120.0, 390.0, 150.0);
        assert_eq!(
            at(AnchorSide::Right, near_right).right(),
            370.0,
            "right flips left"
        );
        let near_left = rect(10.0, 120.0, 30.0, 150.0);
        assert_eq!(
            at(AnchorSide::Left, near_left).left(),
            30.0,
            "left flips right"
        );
    }
    let flipped = placed(
        Placement::BELOW_START.gap(4),
        rect(150.0, 270.0, 250.0, 290.0),
        60.0,
        40.0,
    );
    assert_eq!(
        flipped.bottom(),
        266.0,
        "the gap goes with it to the other side"
    );
}
