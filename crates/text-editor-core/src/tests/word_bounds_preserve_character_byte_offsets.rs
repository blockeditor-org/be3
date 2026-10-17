use super::*;

#[test]
fn word_bounds_preserve_character_byte_offsets() {
    for (text, index, expected) in [
        ("", 0, (0, 0)),
        ("   ", 3, (2, 3)),
        ("\u{2003}\u{2003}", 3, (3, 6)),
        ("café trip", 5, (0, 5)),
        ("café trip", 6, (6, 10)),
        ("café trip", 10, (6, 10)),
        ("don't_stop!", 10, (0, 10)),
        ("!!", 2, (1, 2)),
    ] {
        assert_eq!(word_bounds(text.as_bytes(), index), expected);
    }
    assert_eq!(word_bounds(b"\xff word", 0), (0, 1));
    assert_eq!(word_bounds(b"word\xff", 4), (0, 4));
}
