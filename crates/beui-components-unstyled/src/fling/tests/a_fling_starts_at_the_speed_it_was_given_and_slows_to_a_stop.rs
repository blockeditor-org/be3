use super::*;

#[test]
fn a_fling_starts_at_the_speed_it_was_given_and_slows_to_a_stop() {
    let mut fling = Fling::new(3000.0).expect("a fling");
    let start = fling.velocity();
    assert!(
        (start - 3000.0).abs() < 3000.0 * 0.1,
        "the fling starts near the release speed: {start}"
    );
    let mut previous = start;
    while !fling.done() {
        fling.advance(1.0 / 60.0);
        let velocity = fling.velocity();
        assert!(velocity <= previous + 0.01, "{previous} -> {velocity}");
        previous = velocity;
    }
    assert_eq!(fling.velocity(), 0.0);
}
