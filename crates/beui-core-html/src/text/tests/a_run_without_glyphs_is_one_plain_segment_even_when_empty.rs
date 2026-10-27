use super::*;

#[test]
fn a_run_without_glyphs_is_one_plain_segment_even_when_empty() {
    assert_eq!(segments("", 0, &[]), vec![plain(0..0)]);
    assert_eq!(segments("abc", 7, &[]), vec![plain(0..3)]);
}
