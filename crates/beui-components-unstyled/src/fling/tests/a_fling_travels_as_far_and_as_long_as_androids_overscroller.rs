use super::*;

#[test]
fn a_fling_travels_as_far_and_as_long_as_androids_overscroller() {
    let fling = Fling::new(2000.0).expect("a fling");
    assert!(
        (fling.distance() - 647.0).abs() < 3.0,
        "distance {}",
        fling.distance()
    );
    assert!(
        (fling.duration() - 0.925).abs() < 0.01,
        "duration {}",
        fling.duration()
    );
    let backwards = Fling::new(-2000.0).expect("a fling");
    assert_eq!(backwards.distance(), -fling.distance());

    let mut travelled = Fling::new(2000.0).expect("a fling");
    let mut total = 0.0;
    while !travelled.done() {
        total += travelled.advance(1.0 / 60.0);
    }
    assert!((total - fling.distance()).abs() < 0.01);
}
