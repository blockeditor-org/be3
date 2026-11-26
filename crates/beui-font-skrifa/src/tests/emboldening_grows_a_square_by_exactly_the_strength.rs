use crate::outline::polygon;

#[test]
fn emboldening_grows_a_square_by_exactly_the_strength() {
    let counter_clockwise: &[[f32; 2]] = &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let clockwise: &[[f32; 2]] = &[[0.0, 0.0], [0.0, 10.0], [10.0, 10.0], [10.0, 0.0]];
    for square in [counter_clockwise, clockwise] {
        let mut outline = polygon(&[square]);
        outline.embolden(2.0);
        let mut corners = outline.corners().remove(0);
        corners.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            corners,
            [[0.0, 0.0], [0.0, 12.0], [12.0, 0.0], [12.0, 12.0]],
            "the square grows up and right by the strength, its left side and baseline kept, whichever way it runs"
        );
    }
}
