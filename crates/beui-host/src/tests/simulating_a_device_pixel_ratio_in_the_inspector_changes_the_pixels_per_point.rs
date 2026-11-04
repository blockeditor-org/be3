use super::*;

#[test]
fn simulating_a_device_pixel_ratio_in_the_inspector_changes_the_pixels_per_point() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    harness.rescale(1.5);

    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Simulation);
    harness.frame(vec![]);
    harness.choose_in_simulation("pixel_ratio", 3);
    assert_eq!(harness.frame(vec![]).pixels_per_point(), 2.0);

    harness.toggle_inspector();
    assert_eq!(harness.frame(vec![]).pixels_per_point(), 2.0);

    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Simulation);
    harness.frame(vec![]);
    harness.choose_in_simulation("pixel_ratio", 0);
    assert_eq!(harness.frame(vec![]).pixels_per_point(), 1.5);
}
