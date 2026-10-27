use super::*;

#[test]
fn a_glyph_ending_inside_a_character_stops_before_it() {
    assert_eq!(
        segments("aé", 0, &[glyph(0..2, "x")]),
        vec![shown(0..1, "x"), plain(1..3)]
    );
}
