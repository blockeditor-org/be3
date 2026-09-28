use super::*;

#[test]
fn a_narrow_inspector_puts_its_close_button_beside_its_tabs() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());

    let close = harness.bar_close_rect();
    let last_tab = harness.bar_option_rect(1);
    assert!(
        close.left() >= last_tab.right(),
        "the close button sits to the right of the tabs"
    );
    assert!(
        close.top() < last_tab.bottom() && close.bottom() > last_tab.top(),
        "the close button sits on the row of tabs"
    );
    assert!(
        harness.inspector().document.find_test_id("inspector.close").is_none(),
        "the panel below the tabs does not repeat the close button"
    );

    harness.click(close.center());
    harness.frame(Vec::new());

    assert!(!harness.inspector_open());
}
