use super::*;

#[test]
fn a_deliberate_wall_counts_pushes_only_after_the_pointer_rests_against_it() {
    let mut edges = edges(bottom(true, true));
    let outputs = [display()];

    let mut at = edges.moved(pos2(400.0, 100.0), vec2(0.0, 600.0), 0, &outputs);
    for step in 1..=10 {
        at = edges.moved(at, vec2(0.0, 40.0), step * 8 * MS, &outputs);
    }
    assert_eq!(
        pushed(&edges),
        0.0,
        "a sweep that runs on into the wall never counts"
    );

    let _ = edges.moved(at, vec2(0.0, 25.0), 400 * MS, &outputs);
    assert_eq!(
        pushed(&edges),
        25.0,
        "a push once the pointer has rested there counts"
    );
}
