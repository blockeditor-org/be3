use super::*;

#[test]
fn a_refused_web_view_shows_why_in_its_place() {
    let mut tab = Harness::new();

    tab.editor.web_view_event(
        WebViewId(0),
        WebViewEvent::Failed("This app can't show web pages yet.".into()),
    );
    tab.run();

    assert!(tab.editor.children().is_empty());
    tab.editor
        .snapshot("a_refused_web_view_shows_why_in_its_place");
}
