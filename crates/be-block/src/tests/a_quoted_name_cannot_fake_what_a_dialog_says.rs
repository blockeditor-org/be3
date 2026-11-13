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
}
