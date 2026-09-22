use super::*;

#[test]
fn a_browser_tab_push_discards_forward_history() {
    let visit = |url: &str| {
        BrowserTabOp::Push(HistoryItem {
            url: url.into(),
            title: String::new(),
        })
    };
    let mut tab = BrowserTabContent::default();
    for operation in [
        visit("https://one.example"),
        visit("https://two.example"),
        BrowserTabOp::History(1),
        visit("https://three.example"),
    ] {
        tab.apply(&operation);
    }

    let urls: Vec<_> = tab.history().iter().map(|item| item.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "about:blank",
            "https://one.example",
            "https://three.example"
        ]
    );
    assert_eq!(tab.index(), 2);
    assert!(!tab.can_go_forward());
}
