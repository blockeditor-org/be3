use super::*;

#[test]
fn a_child_placed_in_an_anchored_view_is_placed_in_its_own_points() {
    let mut rig = Rig::new(Some(space()));
    let popover = Rect::from_min_size(pos2(220.0, 130.0), vec2(100.0, 80.0));
    rig.open(vec![layer(3, 0, popover)]);
    *rig.shared.child_in.borrow_mut() = Some(ViewId(3));
    let commit = rig.draw();
    let placed: Vec<_> = commit
        .children
        .iter()
        .flat_map(|placements| {
            placements
                .children
                .iter()
                .map(|child| (placements.view, child.rect))
        })
        .collect();
    assert_eq!(
        placed,
        [(
            ViewId(3),
            Rect::from_min_size(pos2(10.0, 10.0), vec2(30.0, 20.0))
        )],
        "the child is committed in the anchored view's own placements, in its own points"
    );
    assert_eq!(Rig::anchored(&commit).len(), 1, "with the view holding it");
}
