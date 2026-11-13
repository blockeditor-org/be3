use crate::quoted_name;

/// What `quoted_name` makes of `name`, without the quotes and isolates around it.
fn inside(name: &str) -> String {
    let quoted = quoted_name(name);
    quoted
        .strip_prefix("\u{201c}\u{2068}")
        .and_then(|rest| rest.strip_suffix("\u{2069}\u{201d}"))
        .unwrap_or_else(|| panic!("{quoted:?} is quoted and isolated"))
        .to_owned()
}

#[test]
fn a_quoted_name_cannot_fake_what_a_dialog_says() {
    assert_eq!(
        quoted_name("Plans"),
        "\u{201c}\u{2068}Plans\u{2069}\u{201d}",
        "a name is quoted and isolated for bidi"
    );
    assert_eq!(
        inside("X\u{201d} in \u{201c}Private"),
        "X\" in \"Private",
        "a name's curly quotes cannot close the dialog's"
    );
    assert_eq!(
        inside("Team\u{202e}etavirP\nnotes\u{200b}\u{2068}"),
        "TeametavirP notes",
        "bidi overrides and isolates, zero-width marks and newlines go"
    );
    assert_eq!(inside("  a \t  b  "), "a b", "whitespace collapses");
    assert_eq!(
        inside("a\u{3164}\u{3164}b\u{2800}c\u{115f}\u{1160}d"),
        "a b c d",
        "letters that draw as nothing count as spaces"
    );
    let long = inside(&"word ".repeat(40));
    assert_eq!(long.chars().count(), 60, "{long}");
    assert!(long.ends_with('\u{2026}'), "{long}");
    let flags = inside(&format!("{}{}", "a".repeat(58), "\u{1f1eb}\u{1f1f7}\u{1f1e9}\u{1f1ea}"));
    assert!(
        flags.ends_with(&format!("{}\u{1f1eb}\u{1f1f7}\u{2026}", "a")),
        "the cut falls between whole characters as drawn: {flags}"
    );
    assert_eq!(
        inside(
            "\u{2039}a\u{203a} \u{ff02}b\u{ff02} \u{2033}c\u{2033} \u{275d}d\u{275e} \u{301d}e\u{301e}\u{301f}"
        ),
        "'a' \"b\" \"c\" \"d\" \"e\"\"",
        "every quote-like mark is straightened"
    );
    assert_eq!(
        inside("\u{02ee}f\u{02ee} \u{1f676}g\u{1f677} \u{05f4}h\u{05f3} \u{02bc}i\u{02bb} \u{300c}j\u{300f}"),
        "\"f\" \"g\" \"h' 'i' 'j\"",
        "lookalikes of quotes are straightened too"
    );
    assert_eq!(
        inside(&format!("Z{}ok", "\u{0301}".repeat(30))),
        "Z\u{0301}\u{0301}ok",
        "a long run of combining marks is cut"
    );
    assert_eq!(
        inside(&format!("\u{0e01}{}", "\u{0e49}".repeat(20))),
        "\u{0e01}\u{0e49}\u{0e49}",
        "a Thai stack is cut"
    );
    assert_eq!(
        inside(&format!("\u{0410}{}", "\u{0488}\u{0489}".repeat(10))),
        "\u{0410}\u{0488}\u{0489}",
        "Cyrillic enclosing marks are cut"
    );
    assert_eq!(
        inside(&format!("x{}y", "\u{fe0f}".repeat(10))),
        "x\u{fe0f}\u{fe0f}y",
        "variation selectors are cut"
    );
    assert_eq!(inside("Cafe\u{0301}"), "Cafe\u{0301}", "an accent stays");
}
