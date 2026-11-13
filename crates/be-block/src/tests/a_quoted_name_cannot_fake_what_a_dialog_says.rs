use crate::quoted_name;

#[test]
fn a_quoted_name_cannot_fake_what_a_dialog_says() {
    assert_eq!(quoted_name("Plans"), "Plans");
    assert_eq!(
        quoted_name("X\u{201d} in \u{201c}Private"),
        "X\" in \"Private",
        "a name's curly quotes cannot close the dialog's"
    );
    assert_eq!(
        quoted_name("Team\u{202e}etavirP\nnotes\u{200b}"),
        "TeametavirP notes",
        "bidi overrides, zero-width marks and newlines go"
    );
    assert_eq!(quoted_name("  a \t  b  "), "a b", "whitespace collapses");
    let long = quoted_name(&"word ".repeat(40));
    assert_eq!(long.chars().count(), 60, "{long}");
    assert!(long.ends_with('\u{2026}'), "{long}");
    assert_eq!(
        quoted_name(
            "\u{2039}a\u{203a} \u{ff02}b\u{ff02} \u{2033}c\u{2033} \u{275d}d\u{275e} \u{301d}e\u{301e}\u{301f}"
        ),
        "'a' \"b\" \"c\" \"d\" \"e\"\"",
        "every quote-like mark is straightened"
    );
    assert_eq!(
        quoted_name(&format!("Z{}ok", "\u{0301}".repeat(30))),
        "Z\u{0301}\u{0301}ok",
        "a long run of combining marks is cut"
    );
    assert_eq!(
        quoted_name("Cafe\u{0301}"),
        "Cafe\u{0301}",
        "an accent stays"
    );
}
