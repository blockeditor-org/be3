use super::*;

#[test]
fn a_zone_inside_a_held_zone_takes_the_pointer_that_enters_it() {
    let root = RootScope::new();
    let node = origin(&root);
    let zoned = |drop: u32, zone: Rect| Candidate::new(drop, zone, node).zoned(zone);
    // A slot (1) listed before the column (2) around it.
    let slot = Rect::from_min_size(pos2(40.0, 40.0), vec2(20.0, 20.0));
    let column = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 200.0));
    let spec = closest([zoned(1, slot), zoned(2, column)]);

    let empty = frame(&spec, at(pos2(50.0, 150.0)), &DragFrame::default());
    assert_eq!(empty.drop, Some(2), "in the column's empty area, the column");
    let onto = frame(&spec, at(pos2(50.0, 50.0)), &empty.frame);
    assert_eq!(
        onto.drop,
        Some(1),
        "moved onto the slot, the slot, though the column was held and holds the pointer too"
    );
    let back = frame(&spec, at(pos2(50.0, 150.0)), &onto.frame);
    assert_eq!(back.drop, Some(2), "and off it, the column again");
}
