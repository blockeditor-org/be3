use super::*;

#[test]
fn a_child_views_anchor_bounds_never_exceed_its_parents() {
    let parent = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let own = Rect::from_min_size(pos2(700.0, 100.0), vec2(300.0, 200.0));

    assert_eq!(
        bounds_within(parent, own, false),
        parent,
        "a trusted plugin's layers may go wherever its parent's may"
    );
    assert_eq!(
        bounds_within(parent, own, true),
        Rect::from_min_max(pos2(700.0, 100.0), pos2(800.0, 300.0)),
        "a confined plugin's go only in its own box, cut to its parent's bounds"
    );
}
