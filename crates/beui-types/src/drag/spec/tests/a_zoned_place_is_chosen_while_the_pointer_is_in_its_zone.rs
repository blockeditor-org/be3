use super::*;

#[test]
fn a_zoned_place_is_chosen_while_the_pointer_is_in_its_zone() {
    let root = RootScope::new();
    let node = origin(&root);
    // A split's rest is a whole half, far from the band at the edge that chooses it.
    let split = Candidate::new(7, Rect::from_min_size(pos2(500.0, 0.0), vec2(500.0, 1000.0)), node)
        .zoned(Rect::from_min_size(pos2(950.0, 0.0), vec2(50.0, 1000.0)));
    let spec = closest::<u32>([fixed(split), fixed(slot(node, 1, 0.0))]);

    let in_band = frame(&spec, at(pos2(970.0, 500.0)), &DragFrame::default());
    assert_eq!(in_band.drop, Some(7));
    let over_rest = frame(&spec, at(pos2(750.0, 500.0)), &DragFrame::default());
    assert_eq!(
        over_rest.drop,
        Some(1),
        "over its rest but outside its zone, a zoned place is not chosen"
    );
}
