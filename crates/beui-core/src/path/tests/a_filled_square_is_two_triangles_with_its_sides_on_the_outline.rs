use super::*;

#[test]
fn a_filled_square_is_two_triangles_with_its_sides_on_the_outline() {
    let triangles = tessellate(&square(0.0, 10.0), &PathStyle::Fill(FillRule::NonZero), 2.0);

    assert_eq!(triangles.len(), 2);
    assert!((area(&triangles) - 400.0).abs() < 1e-3);
    let outline: u32 = triangles
        .iter()
        .map(|triangle| triangle.edges.count_ones())
        .sum();
    assert_eq!(outline, 4);
}
