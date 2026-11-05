use super::*;

#[test]
fn pushing_past_a_wall_counts_only_after_the_pointer_arrives() {
    let mut edges = edges(bottom(true, false));
    let outputs = [display()];

    let at = edges.moved(pos2(400.0, 500.0), vec2(0.0, 300.0), 0, &outputs);
    assert_eq!(
        at,
        pos2(400.0, 599.0),
        "the wall stops the pointer at the last row"
    );
    assert_eq!(
        pushed(&edges),
        0.0,
        "the motion that carries the pointer to the wall counts for nothing"
    );

    let at = edges.moved(at, vec2(3.0, 10.0), 8 * MS, &outputs);
    assert_eq!(
        at,
        pos2(403.0, 599.0),
        "the pointer still slides along the wall"
    );
    assert_eq!(pushed(&edges), 10.0);

    let at = edges.moved(at, vec2(0.0, -20.0), 16 * MS, &outputs);
    let _ = edges.moved(at, vec2(0.0, 30.0), 24 * MS, &outputs);
    assert_eq!(
        pushed(&edges),
        10.0,
        "leaving the wall and coming back is a new arrival"
    );
}
