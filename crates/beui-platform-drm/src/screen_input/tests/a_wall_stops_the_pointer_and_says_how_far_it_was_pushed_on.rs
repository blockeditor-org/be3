use super::*;

#[test]
fn a_wall_stops_the_pointer_and_says_how_far_it_was_pushed_on() {
    let mut input = input(vec![bottom()], Vec::new());
    let outputs = [display()];

    let at = input.moved(pos2(400.0, 500.0), vec2(0.0, 300.0), &outputs);
    assert_eq!(
        at,
        pos2(400.0, 599.0),
        "the wall stops the pointer at the last row"
    );
    assert!(wall(&input).against, "the pointer rests against it");
    assert_eq!(
        wall(&input).pushed,
        0.0,
        "the pointer was not on the wall when the motion began"
    );

    let at = input.moved(at, vec2(3.0, 10.0), &outputs);
    assert_eq!(
        at,
        pos2(403.0, 599.0),
        "the pointer still slides along the wall"
    );
    assert_eq!(wall(&input).pushed, 10.0, "a push from the wall counts");

    let at = input.moved(at, vec2(0.0, -20.0), &outputs);
    assert!(!wall(&input).against, "the pointer left it");
    let _ = input.moved(at, vec2(0.0, 30.0), &outputs);
    assert_eq!(
        wall(&input).pushed,
        10.0,
        "motion that brings the pointer back to it counts for nothing"
    );

    input.set(
        vec![Wall {
            start: 500.0,
            ..bottom()
        }],
        Vec::new(),
    );
    assert!(
        !wall(&input).against && wall(&input).pushed == 10.0,
        "a wall set again under its id keeps its push, and the pointer is no longer on it"
    );
}
