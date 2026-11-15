use super::*;

#[test]
fn a_zone_left_lets_go_of_its_place() {
    let root = RootScope::new();
    let node = origin(&root);
    let zone = |drop: u32, x: f32| {
        slot(node, drop, x).zoned(Rect::from_min_size(pos2(x, 0.0), vec2(100.0, 100.0)))
    };
    let spec = closest([zone(1, 0.0), zone(2, 200.0)]);
    let inside = frame(&spec, at(pos2(50.0, 50.0)), &DragFrame::default());
    assert_eq!(inside.drop, Some(1));

    let outside = frame(&spec, at(pos2(150.0, 50.0)), &inside.frame);
    assert_eq!(outside.drop, None, "out of every zone, the held place is let go");
    assert!(outside.frame.floating);
}
