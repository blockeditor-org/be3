use super::*;

#[test]
fn an_edge_place_takes_the_corner_up_to_its_diagonal() {
    let root = RootScope::new();
    let node = origin(&root);
    // A wide pane, with bands a fifth deep along its left and bottom edges.
    let pane = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 100.0));
    let left = Candidate::new(1, pane, node)
        .zoned(Rect::from_min_size(pos2(0.0, 0.0), vec2(80.0, 100.0)))
        .nearest_edge(be_lib_geometry::Side::Left, pane);
    let bottom = Candidate::new(2, pane, node)
        .zoned(Rect::from_min_size(pos2(0.0, 80.0), vec2(400.0, 20.0)))
        .nearest_edge(be_lib_geometry::Side::Below, pane);
    let spec = closest([left, bottom]);

    let choose = |pos| frame(&spec, at(pos), &DragFrame::default()).drop;
    assert_eq!(
        choose(pos2(40.0, 95.0)),
        Some(2),
        "a tenth of the width from the left, a twentieth of the height from the bottom: the bottom"
    );
    assert_eq!(
        choose(pos2(10.0, 85.0)),
        Some(1),
        "a fortieth of the width from the left, three twentieths from the bottom: the left"
    );
    assert_eq!(choose(pos2(40.0, 40.0)), Some(1), "away from the corner, its band's");
}
