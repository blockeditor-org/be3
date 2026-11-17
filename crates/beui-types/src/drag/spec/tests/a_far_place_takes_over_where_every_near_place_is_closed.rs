use super::*;

#[test]
fn a_far_place_takes_over_where_every_near_place_is_closed() {
    let root = RootScope::new();
    let node = origin(&root);
    let near = closest([slot(node, 1, 0.0)]).open_where(|point: DragPoint| point.pos.x < 100.0);
    let spec = near.when_far(fixed(slot(node, 2, 500.0)));

    let open = frame(&spec, at(pos2(20.0, 10.0)), &DragFrame::default());
    assert_eq!(open.drop, Some(1), "an open near place within reach is chosen");
    let closed = frame(&spec, at(pos2(120.0, 10.0)), &open.frame);
    assert_eq!(
        closed.drop,
        Some(2),
        "where the near place is closed, it chooses nothing, and the far place takes over however near the item is"
    );
}
