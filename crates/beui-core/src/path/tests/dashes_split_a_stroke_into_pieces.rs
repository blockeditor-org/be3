use super::*;

#[test]
fn dashes_split_a_stroke_into_pieces() {
    let line = Path::polyline([pos2(0.0, 0.0), pos2(40.0, 0.0)]);
    let dashed = tessellate(
        &line,
        &PathStyle::Stroke(Stroke::new(2.0).dashed([5.0, 5.0], 0.0)),
        1.0,
    );

    assert!((area(&dashed) - 40.0).abs() < 1e-2, "{}", area(&dashed));
}
