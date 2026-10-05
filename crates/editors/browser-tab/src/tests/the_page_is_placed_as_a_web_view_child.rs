use super::*;

#[test]
fn the_page_is_placed_as_a_web_view_child() {
    let tab = Harness::new();

    let placed: Vec<ChildContent> = tab
        .editor
        .children()
        .iter()
        .map(|placement| placement.content)
        .collect();
    assert_eq!(placed, [ChildContent::WebView(WebViewId(0))]);
}
