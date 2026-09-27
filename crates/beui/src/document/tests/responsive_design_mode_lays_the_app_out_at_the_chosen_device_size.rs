use super::*;

#[test]
fn responsive_design_mode_lays_the_app_out_at_the_chosen_device_size() {
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    let root = harness.document().root().expect("the column was built");

    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(vec![]);
    harness.click(harness.inspector_center("inspector.simulation.responsive"));
    harness.frame(vec![]);
    harness.assert_screen_size(vec2(390.0, 844.0));

    let shown = harness.shown_screen();
    let toolbar = harness.responsive_control_rect("inspector.responsive.rotate");
    assert!(
        toolbar.bottom() <= shown.top(),
        "the toolbar covers the screen"
    );

    harness.click(toolbar.center());
    harness.frame(vec![]);
    harness.assert_screen_size(vec2(844.0, 390.0));

    harness.click(
        harness
            .responsive_control_rect("inspector.responsive.close")
            .center(),
    );
    harness.frame(vec![]);
    assert!(harness.context.screen_simulation().is_none());
    assert_eq!(
        harness.rect(root).width(),
        WIDE_VIEWPORT.x - harness.inspector().width
    );
}
