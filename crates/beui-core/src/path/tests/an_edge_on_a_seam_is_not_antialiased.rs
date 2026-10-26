use super::*;

#[test]
fn an_edge_on_a_seam_is_not_antialiased() {
    let outline = |paint: &PathPaint| -> u32 {
        paint
            .mesh(2.0)
            .iter()
            .map(|triangle| triangle.edges.count_ones())
            .sum()
    };
    let style = PathStyle::Fill(FillRule::NonZero);
    let whole = PathPaint::new(square(0.0, 10.0), style.clone());
    let seamed = PathPaint::with_seams(
        square(0.0, 10.0),
        style,
        vec![[pos2(10.0, 0.0), pos2(10.0, 10.0)]],
    );

    assert_eq!(outline(&whole), 4);
    assert_eq!(outline(&seamed), 3, "the side on the seam is left hard");
}
