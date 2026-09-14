use super::*;

#[test]
fn the_inspector_keeps_its_native_size_while_a_pixel_ratio_is_simulated() {
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(vec![]);
    let tab = harness.inspector().simulation_tab_node();
    let native = harness
        .inspector()
        .document
        .node_rect(tab)
        .expect("the tab was laid out");

    harness.click(harness.pixel_ratio_option_center(3));
    let output = harness.frame(vec![]);
    assert_eq!(output.pixels_per_point(), 2.0);
    let simulated = harness
        .inspector()
        .document
        .node_rect(tab)
        .expect("the tab was laid out");
    assert_eq!(simulated.size(), native.size());
    let on_screen = simulated.scaled(0.5);
    assert_eq!(on_screen.width(), native.width() / 2.0);
    assert!(output.shapes().iter().any(|shape| matches!(
        shape,
        crate::painter::Shape::Rect { rect, .. } if *rect == on_screen
    )));

    harness.click(harness.pixel_ratio_option_center(0));
    assert_eq!(harness.frame(vec![]).pixels_per_point(), 1.0);
}
