use super::*;

#[test]
fn a_browser_tab_is_named_after_its_page() {
    let blank = BrowserTabContent::default();
    assert_eq!(blank.root().current().url, "about:blank");
    assert_eq!(blank.name(), None);

    let tab = edited(
        &blank,
        [blank
            .root()
            .push(&HistoryItem::new("https://example.com/", ""))],
    );
    let tab = edited(
        &tab,
        [tab.root().replace(&HistoryItem::new(
            "https://example.com/",
            "  Example Domain ",
        ))],
    );

    assert_eq!(tab.name(), Some("Example Domain".to_owned()));
    assert_eq!(BrowserTabContent::decode(&tab.encode()), Ok(tab));
}
