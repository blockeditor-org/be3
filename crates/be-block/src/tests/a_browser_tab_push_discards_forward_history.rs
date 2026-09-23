use super::*;

#[test]
fn a_browser_tab_push_discards_forward_history() {
    let visit = |tab: &BrowserTabContent, url: &str| {
        let edit = tab.root().push(&HistoryItem::new(url, ""));
        edited(tab, [edit])
    };
    let tab = visit(&BrowserTabContent::default(), "about:blank");
    let tab = visit(&tab, "https://one.example");
    let tab = visit(&tab, "https://two.example");
    let tab = edited(&tab, [tab.root().go(1)]);
    assert!(tab.root().can_go_forward());
    let tab = visit(&tab, "https://three.example");

    let root = tab.root();
    let urls: Vec<&str> = root.history.iter().map(|item| item.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "about:blank",
            "https://one.example",
            "https://three.example"
        ]
    );
    assert_eq!(root.index(), 2);
    assert!(!root.can_go_forward());
}
