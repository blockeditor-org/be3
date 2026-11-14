use crate::quotable_name;

#[test]
fn a_quoted_name_cannot_fake_what_a_dialog_says() {
    assert_eq!(
        quotable_name("Plans"),
        "Plans",
        "a plain name is kept as it is: the dialog quotes and isolates it"
    );
    assert_eq!(
        quotable_name("X\u{201d} in \u{201c}Private"),
        "X\" in \"Private",
        "a name's curly quotes cannot close the dialog's"
    );
    assert_eq!(
        quotable_name("Team\u{202e}etavirP\nnotes\u{200b}\u{2068}"),
        "TeametavirP notes",
        "bidi overrides and isolates, zero-width marks and newlines go"
    );
    assert_eq!(quotable_name("  a \t  b  "), "a b", "whitespace collapses");
    assert_eq!(
        quotable_name("a\u{3164}\u{3164}b\u{2800}c\u{115f}\u{1160}d"),
        "a b c d",
        "letters that draw as nothing count as spaces"
    );
    let long = quotable_name(&"word ".repeat(40));
    assert_eq!(long.chars().count(), 60, "{long}");
    assert!(long.ends_with('\u{2026}'), "{long}");
    let flags = quotable_name(&format!(
        "{}{}",
        "a".repeat(57),
        "\u{1f1eb}\u{1f1f7}\u{1f1e9}\u{1f1ea}"
    ));
    assert_eq!(
        flags,
        format!("{}\u{1f1eb}\u{1f1f7}\u{2026}", "a".repeat(57)),
        "the cut falls between whole characters as drawn: {flags}"
    );
    assert_eq!(
        quotable_name(
            "\u{2039}a\u{203a} \u{ff02}b\u{ff02} \u{2033}c\u{2033} \u{275d}d\u{275e} \u{301d}e\u{301e}\u{301f}"
        ),
        "'a' \"b\" \"c\" \"d\" \"e\"\"",
        "every quote-like mark is straightened"
    );
    assert_eq!(
        quotable_name(
            "\u{02ee}f\u{02ee} \u{1f676}g\u{1f677} \u{05f4}h\u{05f3} \u{02bc}i\u{02bb} \u{300c}j\u{300f}"
        ),
        "\"f\" \"g\" \"h' 'i' 'j\"",
        "lookalikes of quotes are straightened too"
    );
    assert_eq!(
        quotable_name(&format!("Z{}ok", "\u{0301}".repeat(30))),
        "Z\u{0301}\u{0301}ok",
        "a long run of combining marks is cut"
    );
    assert_eq!(
        quotable_name(&format!("\u{0e01}{}", "\u{0e49}".repeat(20))),
        "\u{0e01}\u{0e49}\u{0e49}",
        "a Thai stack is cut"
    );
    assert_eq!(
        quotable_name(&format!(
            "\u{0e01}{}",
            "\u{0e49}\u{0e49}\u{200b}".repeat(20)
        )),
        "\u{0e01}\u{0e49}\u{0e49}",
        "characters dropped between marks do not start a new stack"
    );
    assert_eq!(
        quotable_name(&format!("\u{0410}{}", "\u{0488}\u{0489}".repeat(10))),
        "\u{0410}\u{0488}\u{0489}",
        "Cyrillic enclosing marks are cut"
    );
    assert_eq!(
        quotable_name(&format!("x{}y", "\u{fe0f}".repeat(10))),
        "x\u{fe0f}\u{fe0f}y",
        "variation selectors are cut"
    );
    assert_eq!(quotable_name("Cafe\u{0301}"), "Cafe\u{0301}", "an accent stays");
}
