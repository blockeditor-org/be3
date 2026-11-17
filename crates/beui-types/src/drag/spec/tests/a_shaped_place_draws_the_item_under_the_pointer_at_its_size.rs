use super::*;

#[test]
fn a_shaped_place_draws_the_item_under_the_pointer_at_its_size() {
    let root = RootScope::new();
    let node = origin(&root);
    // A wide place chosen by its zone; the 20-point item is held by its centre.
    let pane = Candidate::new(
        1,
        Rect::from_min_size(pos2(100.0, 0.0), vec2(200.0, 100.0)),
        node,
    )
    .zoned(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0)));
    let spec = closest([pane]).shaped();

    let shown = frame(&spec, at(pos2(50.0, 60.0)), &DragFrame::default());
    assert_eq!(shown.drop, Some(1), "it chooses the place its spec chooses");
    assert!(!shown.frame.floating);
    assert_eq!(
        shown.frame.item,
        Rect::from_min_size(pos2(-50.0, 10.0), vec2(200.0, 100.0)),
        "drawn the size of the place, with the pointer at its middle as it held the item's"
    );

    let away = frame(&spec, at(pos2(500.0, 500.0)), &shown.frame);
    assert_eq!(away.drop, None);
    assert_eq!(
        away.frame.item.size(),
        vec2(20.0, 20.0),
        "floating, it keeps its own size"
    );
}
