use super::*;

#[test]
fn the_pointer_is_near_an_open_edge_inside_its_keep() {
    let outputs = [display()];
    let mut edges = edges(bottom(true, false));
    edges.placed(pos2(400.0, 580.0));
    assert!(!edges.states()[0].near, "a closed edge keeps nothing near");

    edges.set(vec![ScreenEdge {
        keep: Some(Rect::from_min_size(pos2(0.0, 540.0), vec2(800.0, 60.0))),
        ..bottom(false, false)
    }]);
    assert!(
        edges.states()[0].near,
        "opening it finds the pointer already near"
    );

    let _ = edges.moved(pos2(400.0, 580.0), vec2(0.0, -200.0), 0, &outputs);
    assert!(!edges.states()[0].near);
}
