use super::*;

#[test]
fn closest_snaps_to_the_nearest_rest_and_holds_it_until_another_is_clearly_nearer() {
    let root = RootScope::new();
    let node = origin(&root);
    let spec = closest([slot(node, 1, 0.0), slot(node, 2, 100.0)]);

    let first = frame(&spec, at(pos2(30.0, 10.0)), &DragFrame::default());
    assert_eq!(first.drop, Some(1));
    assert_eq!(first.frame.item.min, pos2(0.0, 0.0), "the item snaps to the rest");
    assert!(!first.frame.floating);

    let past_middle = frame(&spec, at(pos2(62.0, 10.0)), &first.frame);
    assert_eq!(
        past_middle.drop,
        Some(1),
        "just past the middle, the place chosen holds by its stickiness"
    );
    let clear = frame(&spec, at(pos2(80.0, 10.0)), &past_middle.frame);
    assert_eq!(clear.drop, Some(2), "clearly nearer, the other place takes over");
    assert_eq!(clear.frame.item.min, pos2(100.0, 0.0));
}
