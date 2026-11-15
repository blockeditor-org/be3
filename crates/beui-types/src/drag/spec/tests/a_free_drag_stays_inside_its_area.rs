use super::*;

#[test]
fn a_free_drag_stays_inside_its_area() {
    let root = RootScope::new();
    let node = origin(&root);
    let spec = free(node, |rect: Rect| rect.min.x as u32);

    let inside = frame(&spec, at(pos2(300.0, 300.0)), &DragFrame::default());
    assert_eq!(inside.frame.item.center(), pos2(300.0, 300.0));
    assert_eq!(inside.drop, Some(290), "the drop is named from where the item rests");
    let outside = frame(&spec, at(pos2(-50.0, 1200.0)), &inside.frame);
    assert_eq!(
        outside.frame.item.min,
        pos2(0.0, 980.0),
        "past the edges the item stops at them"
    );
}
