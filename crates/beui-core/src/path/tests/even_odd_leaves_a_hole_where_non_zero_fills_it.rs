use super::*;

#[test]
fn even_odd_leaves_a_hole_where_non_zero_fills_it() {
    let mut nested = square(0.0, 10.0).elements().to_vec();
    nested.extend_from_slice(square(2.0, 8.0).elements());
    let nested = Path { elements: nested };

    let non_zero = tessellate(&nested, &PathStyle::Fill(FillRule::NonZero), 1.0);
    let even_odd = tessellate(&nested, &PathStyle::Fill(FillRule::EvenOdd), 1.0);

    assert!((area(&non_zero) - 100.0).abs() < 1e-3);
    assert!((area(&even_odd) - 64.0).abs() < 1e-3);
}
