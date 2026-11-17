use super::*;

#[test]
fn a_beside_overlay_lines_up_with_its_anchor_on_every_side_and_alignment() {
    let anchor = rect(150.0, 120.0, 250.0, 150.0);
    let expected = |side: AnchorSide, align: AnchorAlign| {
        let (x, y) = match (side, align) {
            (AnchorSide::Below, AnchorAlign::Start) => (150.0, 150.0),
            (AnchorSide::Below, AnchorAlign::Center) => (170.0, 150.0),
            (AnchorSide::Below, AnchorAlign::End) => (190.0, 150.0),
            (AnchorSide::Above, AnchorAlign::Start) => (150.0, 80.0),
            (AnchorSide::Above, AnchorAlign::Center) => (170.0, 80.0),
            (AnchorSide::Above, AnchorAlign::End) => (190.0, 80.0),
            (AnchorSide::Right, AnchorAlign::Start) => (250.0, 120.0),
            (AnchorSide::Right, AnchorAlign::Center) => (250.0, 115.0),
            (AnchorSide::Right, AnchorAlign::End) => (250.0, 110.0),
            (AnchorSide::Left, AnchorAlign::Start) => (90.0, 120.0),
            (AnchorSide::Left, AnchorAlign::Center) => (90.0, 115.0),
            (AnchorSide::Left, AnchorAlign::End) => (90.0, 110.0),
        };
        Rect::from_min_size(pos2(x, y), Vec2::new(60.0, 40.0))
    };
    for side in SIDES {
        for align in ALIGNS {
            assert_eq!(
                placed(placement(side, align), anchor, 60.0, 40.0),
                expected(side, align),
                "{side:?} {align:?} opens on its side, lined up by its alignment"
            );
        }
    }
    assert_eq!(Placement::BELOW_END, placement(AnchorSide::Below, AnchorAlign::End));
    assert_eq!(Placement::LEFT_START, placement(AnchorSide::Left, AnchorAlign::Start));
    assert_eq!(Placement::ABOVE_START, placement(AnchorSide::Above, AnchorAlign::Start));
}
