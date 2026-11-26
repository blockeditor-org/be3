use crate::Query;

#[test]
fn a_phrase_matches_only_where_its_words_are_in_a_row() {
    let text = "The river was low. Further on, the Big\nRiver met the big river delta.";
    let query = Query::parse("\"big river\"");
    let marked: Vec<&str> = query
        .matches(text)
        .into_iter()
        .map(|range| &text[range])
        .collect();
    assert_eq!(
        marked,
        ["Big\nRiver", "big river"],
        "a phrase's word alone is no match, so the first match is where the phrase is"
    );
    let snippet = query.snippet(text);
    assert_eq!(&snippet.text[snippet.matches[0].clone()], "Big River");

    let overlapping = Query::parse("river \"big river\" delta");
    let marked: Vec<&str> = overlapping
        .matches(text)
        .into_iter()
        .map(|range| &text[range])
        .collect();
    assert_eq!(
        marked,
        ["river", "Big\nRiver", "big river", "delta"],
        "a word inside a phrase's match joins it rather than overlapping it"
    );
}
