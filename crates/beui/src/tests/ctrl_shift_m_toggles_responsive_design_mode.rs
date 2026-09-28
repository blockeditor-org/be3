use super::*;

#[test]
fn ctrl_shift_m_toggles_responsive_design_mode() {
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);

    harness.toggle_responsive();
    harness.frame(vec![]);
    assert!(harness.context.screen_simulation().is_some());
    assert!(harness.document().shown_screen().is_some());

    harness.toggle_responsive();
    harness.frame(vec![]);
    assert!(harness.context.screen_simulation().is_none());
    assert!(harness.document().shown_screen().is_none());
}
