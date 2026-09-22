use super::*;

#[test]
fn a_pushed_url_becomes_the_tab_s_history() {
    let mut tab = Harness::new();

    tab.host
        .push_web_view_event(WebViewEvent::Push("https://example.com/next".into()));
    tab.run();

    assert_eq!(
        tab.urls(),
        ["https://example.com/", "https://example.com/next"]
    );
}
