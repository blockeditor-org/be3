use crate::is_grapheme_boundary;

#[test]
fn long_context() {
    let extends = "\u{301}".repeat(2048);
    let emoji = format!("a\n👩{extends}\u{200d}👩z");
    let right = emoji.len() - "👩z".len();
    assert!(!is_grapheme_boundary(emoji.as_bytes(), right));
    let non_emoji = format!("a{extends}\u{200d}👩");
    assert!(is_grapheme_boundary(
        non_emoji.as_bytes(),
        non_emoji.len() - "👩".len()
    ));
    let indic = format!("क{extends}्{extends}क");
    assert!(!is_grapheme_boundary(
        indic.as_bytes(),
        indic.len() - "क".len()
    ));
    let no_linker = format!("क{extends}क");
    assert!(is_grapheme_boundary(
        no_linker.as_bytes(),
        no_linker.len() - "क".len()
    ));
    let flags = format!("a{}z", "🇦".repeat(2049));
    for count in [2047, 2048] {
        assert_eq!(
            is_grapheme_boundary(flags.as_bytes(), 1 + count * 4),
            count % 2 == 0
        );
    }
    let line = format!("{}é{}", "a".repeat(100_000), "b".repeat(100_000));
    assert!(is_grapheme_boundary(line.as_bytes(), 100_000));
    assert!(!is_grapheme_boundary(line.as_bytes(), 100_001));
    assert!(is_grapheme_boundary(line.as_bytes(), 100_002));
}
