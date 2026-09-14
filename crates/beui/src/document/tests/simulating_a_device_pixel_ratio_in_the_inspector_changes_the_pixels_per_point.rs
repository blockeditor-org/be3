use super::*;

#[test]
fn simulating_a_device_pixel_ratio_in_the_inspector_changes_the_pixels_per_point() {
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    harness.context.set_pixels_per_point(1.5);

    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(vec![]);
    harness.click(harness.pixel_ratio_option_center(3));
    assert_eq!(harness.frame(vec![]).pixels_per_point(), 2.0);

    harness.toggle_inspector();
    assert_eq!(harness.frame(vec![]).pixels_per_point(), 2.0);

    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(vec![]);
    harness.click(harness.pixel_ratio_option_center(0));
    assert_eq!(harness.frame(vec![]).pixels_per_point(), 1.5);
}
