use super::*;

#[test]
fn a_drag_floats_beyond_the_reach_of_its_near_places() {
    let root = RootScope::new();
    let node = origin(&root);
    let spec = closest([slot(node, 1, 0.0)]).when_far(floating());

    let near = frame(&spec, at(pos2(40.0, 10.0)), &DragFrame::default());
    assert_eq!(near.drop, Some(1));
    let far = frame(&spec, at(pos2(10.0, 10.0 + REACH + 20.0)), &near.frame);
    assert_eq!(far.drop, None, "beyond the reach it chooses nothing");
    assert!(far.frame.floating);
    assert_eq!(
        far.frame.item.center(),
        pos2(10.0, 10.0 + REACH + 20.0),
        "and floats under the pointer"
    );
}
