use crate::outline::polygon;

#[test]
fn emboldening_shrinks_a_counter() {
    let outer: &[[f32; 2]] = &[[0.0, 0.0], [0.0, 20.0], [20.0, 20.0], [20.0, 0.0]];
    let counter: &[[f32; 2]] = &[[5.0, 5.0], [15.0, 5.0], [15.0, 15.0], [5.0, 15.0]];
    let mut outline = polygon(&[outer, counter]);
    outline.embolden(2.0);
    let corners = outline.corners();
    let span = |points: &[[f32; 2]], axis: usize| {
        let values = points.iter().map(|point| point[axis]);
        values.clone().fold(f32::MIN, f32::max) - values.fold(f32::MAX, f32::min)
    };
    assert_eq!(
        (span(&corners[0], 0), span(&corners[0], 1)),
        (22.0, 22.0),
        "the ink's outer edge grows by the strength"
    );
    assert_eq!(
        (span(&corners[1], 0), span(&corners[1], 1)),
        (8.0, 8.0),
        "the counter inside it shrinks by the strength"
    );
}
