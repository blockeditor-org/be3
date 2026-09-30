use super::*;
use crate::filter::ColorVision;

#[test]
fn the_simulate_tab_filters_the_document_without_the_screen_reader() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(Vec::new());

    assert!(harness.frame(Vec::new()).filter().is_none());

    harness.drag_simulation_slider("blur", 1.0);
    harness.drag_simulation_slider("contrast", 0.5);
    harness.click(harness.color_vision_option_center(1));
    let output = harness.frame(Vec::new());

    let filter = output.filter().expect("the simulate tab set no filter");
    assert!(filter.blur > 0.0, "the blur slider read {}", filter.blur);
    assert!(
        filter.contrast < 1.0 && filter.contrast > 0.0,
        "the contrast slider read {}",
        filter.contrast
    );
    assert_eq!(filter.vision, ColorVision::Protanopia);

    let panel = harness.inspector().panel_width(
        &harness.context,
        Rect::from_min_size(Pos2::ZERO, TALL_VIEWPORT),
    );
    assert!(
        filter.region.right() <= TALL_VIEWPORT.x - panel,
        "the filter reached into the inspector panel"
    );
}
