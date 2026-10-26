use super::*;

#[test]
fn the_inspector_keeps_its_native_size_while_a_pixel_ratio_is_simulated() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(vec![]);
    let keyboard = harness.inspector().find("inspector.simulation.keyboard");
    let native = harness
        .inspector()
        .document
        .node_rect(keyboard)
        .expect("the button was laid out");

    harness.choose_in_simulation("pixel_ratio", 3);
    let output = harness.frame(vec![]);
    assert_eq!(output.pixels_per_point(), 2.0);
    let simulated = harness
        .inspector()
        .document
        .node_rect(keyboard)
        .expect("the button was laid out");
    assert_eq!(simulated.size(), native.size());
    let on_screen = simulated.scaled(0.5);
    assert_eq!(on_screen.width(), native.width() / 2.0);
    assert!(output.main().shapes().iter().any(|shape| matches!(
        shape,
        crate::painter::Shape::Rect { rect, .. } if *rect == on_screen
    )));

    harness.choose_in_simulation("pixel_ratio", 0);
    assert_eq!(harness.frame(vec![]).pixels_per_point(), 1.0);
}
