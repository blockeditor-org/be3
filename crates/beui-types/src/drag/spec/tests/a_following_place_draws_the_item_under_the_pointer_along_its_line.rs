use super::*;

#[test]
fn a_following_place_draws_the_item_under_the_pointer_along_its_line() {
    let root = RootScope::new();
    let node = origin(&root);
    // Slots along x at y 0; the item is held 30 below them, nearer the second.
    let spec = closest([slot(node, 1, 0.0), slot(node, 2, 40.0)])
        .following(be_lib_geometry::Direction::Horizontal)
        .when_far(floating());

    let near = frame(&spec, at(pos2(35.0, 40.0)), &DragFrame::default());
    assert_eq!(near.drop, Some(2), "it chooses the place its spec chooses");
    assert!(!near.frame.floating);
    assert_eq!(
        near.frame.item.min,
        pos2(25.0, 0.0),
        "drawn under the pointer along the line, and on the place's line across it"
    );

    // Held on the first place, then moved 26 along and 60 across: along the line the
    // second is 12 nearer, beyond the stickiness, though in all it is only 4 nearer.
    let held = frame(&spec, at(pos2(10.0, 10.0)), &DragFrame::default());
    assert_eq!(held.drop, Some(1));
    let across = frame(&spec, at(pos2(36.0, 70.0)), &held.frame);
    assert_eq!(
        across.drop,
        Some(2),
        "it chooses by how far along the line a place is, however far across it the pointer is"
    );
}
