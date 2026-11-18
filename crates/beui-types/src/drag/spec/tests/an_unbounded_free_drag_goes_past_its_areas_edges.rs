use super::*;

#[test]
fn an_unbounded_free_drag_goes_past_its_areas_edges() {
    let root = RootScope::new();
    let node = origin(&root);
    let spec = free(node, |rect: Rect| rect.min.x as i32 as u32).unbounded();

    let outside = frame(&spec, at(pos2(1200.0, -50.0)), &DragFrame::default());
    assert_eq!(
        outside.frame.item.center(),
        pos2(1200.0, -50.0),
        "the item stays under the pointer past the area's edges"
    );
    assert_eq!(
        outside.drop,
        Some(1190),
        "the drop is named from where the item is, outside the area"
    );
    assert!(moves_freely(&closest([fixed(slot(node, 1, 0.0))]).when_far(spec)));
    assert!(!moves_freely(&closest([fixed(slot(node, 1, 0.0))])));
}
