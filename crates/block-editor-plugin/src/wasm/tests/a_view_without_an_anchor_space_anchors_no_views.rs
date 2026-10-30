use super::*;

#[test]
fn a_view_without_an_anchor_space_anchors_no_views() {
    let mut rig = Rig::new(None);
    rig.open(vec![layer(
        1,
        0,
        Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0)),
    )]);
    let commit = rig.draw();
    assert_eq!(rig.shared.seen.borrow().spaces, [None]);
    assert!(
        Rig::anchored(&commit).is_empty(),
        "no anchored view is reported"
    );
    assert!(rig.shared.seen.borrow().painted.is_empty());
    assert!(rig.anchored_screens().is_empty());
}
