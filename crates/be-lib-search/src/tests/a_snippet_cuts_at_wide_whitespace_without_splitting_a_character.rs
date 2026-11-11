use crate::Query;

#[test]
fn a_snippet_cuts_at_wide_whitespace_without_splitting_a_character() {
    for space in ["\u{a0}", "\u{3000}", "\u{2003}", " "] {
        let text = format!("{}{space}{}", "漢字é".repeat(20), "x".repeat(4))
            + &"é".repeat(70)
            + &format!(" {space}{space}needle{space}tail");
        let snippet = Query::parse("needle").snippet(&text);
        let marked: Vec<&str> = snippet
            .matches
            .iter()
            .map(|range| &snippet.text[range.clone()])
            .collect();
        assert_eq!(marked, ["needle"], "{space:?}: {}", snippet.text);
        let long = format!("{}{space}needle", "é".repeat(61));
        let snippet = Query::parse("needle").snippet(&long);
        assert!(snippet.text.ends_with("needle"), "{space:?}");
    }
}
