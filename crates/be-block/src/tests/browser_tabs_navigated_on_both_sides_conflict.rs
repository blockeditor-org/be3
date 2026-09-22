use super::*;

#[test]
fn browser_tabs_navigated_on_both_sides_conflict() {
    let base = BrowserTabContent::at("https://example.com/");
    let visit = |url: &str| {
        let mut tab = base.clone();
        tab.apply(&BrowserTabOp::Push(HistoryItem {
            url: url.into(),
            title: String::new(),
        }));
        tab
    };
    let ours = visit("https://example.com/ours");
    let theirs = visit("https://example.com/theirs");

    assert_eq!(
        BrowserTabContent::merge3(&base, &base, &theirs),
        MergeResult::Clean(theirs.clone())
    );
    assert_eq!(
        BrowserTabContent::merge3(&base, &ours, &theirs),
        MergeResult::Conflicted {
            value: ours,
            conflicts: 1,
        }
    );
}
