use super::*;

#[test]
fn a_fling_into_the_edge_bounces_back_without_passing_it() {
    let frame = 1.0 / 120.0;
    let (mut stretch, mut velocity) = (-1.0, -3000.0);
    bounce(&mut stretch, &mut velocity, frame);
    let entered = (stretch + 1.0) / frame;
    assert!(
        entered < -2900.0,
        "the content crosses the edge at the speed it arrived: {entered}"
    );

    let mut deepest = stretch;
    for _ in 0..240 {
        let back = bounce(&mut stretch, &mut velocity, frame);
        assert_eq!(back, 0.0, "a fling bouncing back comes to rest at the edge");
        assert!(stretch <= 0.0, "it never springs past the edge: {stretch}");
        deepest = deepest.min(stretch);
    }
    assert!(
        deepest < -150.0 && deepest > -300.0,
        "it travels into the band as far as its speed takes it: {deepest}"
    );
    assert_eq!((stretch, velocity), (0.0, 0.0));
}
