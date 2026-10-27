use super::*;

#[test]
fn a_glyph_belongs_to_the_run_it_starts_in_and_is_cut_at_that_runs_end() {
    let glyphs = [glyph(2..6, "·")];
    assert_eq!(
        segments("abcd", 0, &glyphs),
        vec![plain(0..2), shown(2..4, "·")]
    );
    assert_eq!(segments("efgh", 4, &glyphs), vec![plain(0..4)]);
}
