use super::*;

#[test]
fn a_browser_tab_is_named_after_its_page_and_refuses_a_bad_index() {
    let mut tab = BrowserTabContent::default();
    assert_eq!(tab.name(), None);

    tab.apply(&BrowserTabOp::Push(HistoryItem {
        url: "https://example.com/".into(),
        title: String::new(),
    }));
    tab.apply(&BrowserTabOp::Replace(HistoryItem {
        url: "https://example.com/".into(),
        title: "  Example Domain ".into(),
    }));
    tab.apply(&BrowserTabOp::History(9));

    assert_eq!(tab.index(), 1);
    assert_eq!(tab.name(), Some("Example Domain".to_owned()));
    assert_eq!(BrowserTabContent::decode(&tab.encode()), Ok(tab.clone()));

    let mut bytes = tab.encode();
    let last = bytes.len() - 1;
    bytes[last] = 7;
    assert_eq!(
        BrowserTabContent::decode(&bytes),
        Err(ContentError::Malformed(
            "a browser tab points past its history"
        ))
    );
}
