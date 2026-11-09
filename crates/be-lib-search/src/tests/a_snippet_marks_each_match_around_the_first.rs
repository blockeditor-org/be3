use crate::Query;

#[test]
fn a_snippet_marks_each_match_around_the_first() {
    let text = "Notes from the offsite. ".repeat(6)
        + "We agreed the Launch moves to Friday, and the launch party after it. "
        + &"More notes follow here. ".repeat(6);
    let snippet = Query::parse("launch fri").snippet(&text);
    assert!(snippet.text.starts_with('…'), "{}", snippet.text);
    assert!(snippet.text.ends_with('…'), "{}", snippet.text);
    let marked: Vec<&str> = snippet
        .matches
        .iter()
        .map(|range| &snippet.text[range.clone()])
        .collect();
    assert_eq!(marked, ["Launch", "Friday", "launch"]);

    let short = Query::parse("\"big river\"").snippet("The Big River runs");
    assert_eq!(short.text, "The Big River runs");
    let marked: Vec<&str> = short
        .matches
        .iter()
        .map(|range| &short.text[range.clone()])
        .collect();
    assert_eq!(marked, ["Big", "River"]);

    let none = Query::parse("absent").snippet("Only the name matched\nhere");
    assert_eq!(none.text, "Only the name matched here");
    assert!(none.matches.is_empty());
}
