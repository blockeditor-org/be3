use super::*;

#[test]
fn glyphs_that_overlap_or_cover_nothing_are_left_out() {
    assert_eq!(
        segments(
            "abcdef",
            0,
            &[glyph(1..4, "x"), glyph(2..5, "y"), glyph(5..5, "z")]
        ),
        vec![plain(0..1), shown(1..4, "x"), plain(4..6)]
    );
}
