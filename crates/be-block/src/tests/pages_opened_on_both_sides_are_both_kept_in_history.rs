use super::*;

#[test]
fn pages_opened_on_both_sides_are_both_kept_in_history() {
    let empty = BrowserTabContent::default();
    let base = edited(
        &empty,
        [empty
            .root()
            .push(&HistoryItem::new("https://start.example", "Start"))],
    );
    let ours = edited(
        &base,
        [base
            .root()
            .push(&HistoryItem::new("https://ours.example", "Ours"))],
    );
    let theirs = edited(
        &base,
        [base
            .root()
            .push(&HistoryItem::new("https://theirs.example", "Theirs"))],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1, "both sides moved the current page");
    let tab = merged.root();
    assert_eq!(tab.current().url, "https://ours.example");
    let urls: Vec<String> = tab.history.iter().map(|item| item.url.clone()).collect();
    assert_eq!(urls.len(), 3);
    assert!(urls.contains(&"https://theirs.example".to_owned()));
}
