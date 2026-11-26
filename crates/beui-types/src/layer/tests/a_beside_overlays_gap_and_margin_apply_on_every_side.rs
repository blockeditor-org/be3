use super::*;

#[test]
fn a_beside_overlays_gap_and_margin_apply_on_every_side() {
    let anchor = rect(150.0, 120.0, 250.0, 150.0);
    let gapped = |placement: Placement| placed(placement.gap(4), anchor, 60.0, 40.0);
    assert_eq!(gapped(Placement::BELOW_START).top(), 154.0);
    assert_eq!(gapped(Placement::ABOVE_START).bottom(), 116.0);
    assert_eq!(gapped(Placement::RIGHT_START).left(), 254.0);
    assert_eq!(gapped(Placement::LEFT_START).right(), 146.0);
    let edge = rect(360.0, 0.0, 400.0, 20.0);
    let kept = |placement: Placement| placed(placement.margin(8), edge, 60.0, 40.0);
    assert_eq!(
        kept(Placement::BELOW_START).right(),
        392.0,
        "kept off the right edge"
    );
    assert_eq!(
        kept(Placement::LEFT_START).top(),
        8.0,
        "kept off the top edge"
    );
    assert_eq!(
        Placement::Center.gap(4).margin(8),
        Placement::Center,
        "only a placement beside its anchor has a gap and margin"
    );
}
