use super::*;

#[test]
fn the_app_tab_shows_the_document_below_the_tab_bar() {
    let HelloColumn {
        document, padding, ..
    } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.click(harness.bar_option_center(0));
    harness.frame(Vec::new());

    let content = harness.rect(padding);
    assert_eq!(content.width(), VIEWPORT.x);
    assert!(content.top() >= harness.bar_option_rect(0).bottom());
}
