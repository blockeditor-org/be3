use super::*;

#[test]
fn glyphs_cut_a_run_at_the_bytes_they_cover_counted_from_the_text_start() {
    let text = "a b\tc";
    assert_eq!(
        segments(text, 10, &[glyph(11..12, "·"), glyph(13..14, "→")]),
        vec![
            plain(0..1),
            shown(1..2, "·"),
            plain(2..3),
            shown(3..4, "→"),
            plain(4..5)
        ]
    );
}
