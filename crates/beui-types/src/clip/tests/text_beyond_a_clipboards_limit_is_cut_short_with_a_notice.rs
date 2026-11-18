use super::*;

#[test]
fn text_beyond_a_clipboards_limit_is_cut_short_with_a_notice() {
    assert_eq!(
        cut_short("short", 5, " [cut]"),
        "short",
        "text within the limit stays whole"
    );

    let text = "a😀b".repeat(4);
    let cut = cut_short(&text, 10, "…");
    assert_eq!(
        cut, "a😀ba😀ba…",
        "cut at a character, never inside a surrogate pair"
    );
    assert!(utf16_units(&cut) <= 10);

    let wide = "😀".repeat(3);
    assert_eq!(
        cut_short(&wide, 4, "…"),
        "😀…",
        "a character that would overrun what is left after the notice is left out"
    );
}
