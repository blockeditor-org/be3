use super::*;

#[test]
fn a_stroke_is_filled_as_an_outline_around_its_path() {
    let line = Path::polyline([pos2(0.0, 0.0), pos2(10.0, 0.0)]);
    let butt = tessellate(&line, &PathStyle::Stroke(Stroke::new(2.0)), 1.0);
    let square = tessellate(
        &line,
        &PathStyle::Stroke(Stroke::new(2.0).cap(LineCap::Square)),
        1.0,
    );

    assert!((area(&butt) - 20.0).abs() < 1e-2, "{}", area(&butt));
    assert!((area(&square) - 24.0).abs() < 1e-2, "{}", area(&square));
}
