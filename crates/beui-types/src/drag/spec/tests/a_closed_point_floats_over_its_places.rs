use super::*;

#[test]
fn a_closed_point_floats_over_its_places() {
    let root = RootScope::new();
    let node = origin(&root);
    let zone = |drop: u32, x: f32| {
        Candidate::new(drop, Rect::from_min_size(pos2(x, 0.0), vec2(100.0, 100.0)), node)
            .zoned(Rect::from_min_size(pos2(x, 0.0), vec2(100.0, 100.0)))
    };
    // Something covers the first place's left half.
    let covered = closest([zone(1, 0.0)]).open_where(|point: DragPoint| point.pos.x >= 50.0);
    let spec = closest([covered, closest([zone(2, 100.0)])]);

    let open = frame(&spec, at(pos2(70.0, 50.0)), &DragFrame::default());
    assert_eq!(open.drop, Some(1), "an open point reaches the place under it");

    let closed = frame(&spec, at(pos2(20.0, 50.0)), &open.frame);
    assert_eq!(closed.drop, None, "a covered point reaches no place");
    assert!(closed.frame.floating);

    let after = frame(&spec, at(pos2(150.0, 50.0)), &closed.frame);
    assert_eq!(
        after.frame.chosen,
        frame(&spec, at(pos2(150.0, 50.0)), &DragFrame::default()).frame.chosen,
        "places after it keep their ids, open or closed"
    );
    assert_eq!(after.drop, Some(2));
}
