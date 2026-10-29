use super::*;

#[test]
fn a_layer_a_child_in_a_layer_opens_takes_presses_over_that_child() {
    let popover = Rect::from_min_size(pos2(0.0, 100.0), vec2(260.0, 240.0));
    let picker = Rect::from_min_size(pos2(8.0, 120.0), vec2(250.0, 220.0));
    let menu = Rect::from_min_size(pos2(8.0, 120.0), vec2(250.0, 100.0));
    let layers = vec![shown(1, 1, 4, popover), shown(2, 1, 5, menu)];
    let holes = [(1, ViewId(1), picker)];
    let takes = taken(&layers, &holes);

    assert!(
        !takes.contains(pos2(100.0, 300.0)),
        "the popover gives presses over the active child it holds to that child"
    );
    assert!(
        takes.contains(pos2(100.0, 150.0)),
        "but the child's own menu, opened after, takes them where it is"
    );
    assert!(
        takes.contains(pos2(2.0, 110.0)),
        "and the popover keeps its edge"
    );
}
