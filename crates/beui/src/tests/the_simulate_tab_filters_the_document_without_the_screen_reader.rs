use super::*;
use crate::filter::ColorVision;

#[test]
fn the_simulate_tab_filters_the_document_without_the_screen_reader() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Simulation);
    harness.frame(Vec::new());

    assert!(harness.frame(Vec::new()).main().filters().is_empty());

    harness.drag_simulation_slider("blur", 1.0);
    harness.drag_simulation_slider("contrast", 0.5);
    harness.choose_in_simulation("color_vision", ColorVision::Protanopia);
    let output = harness.frame(Vec::new());

    let filters = output.main().filters();
    let [filter] = filters.as_slice() else {
        panic!("the simulate tab set {} filters", filters.len());
    };
    assert!(filter.blur > 0.0, "the blur slider read {}", filter.blur);
    assert!(
        filter.contrast < 1.0 && filter.contrast > 0.0,
        "the contrast slider read {}",
        filter.contrast
    );
    assert_eq!(filter.vision, ColorVision::Protanopia);

    assert!(
        filter.region.right() <= harness.app_rect().right(),
        "the filter reached into the inspector panel"
    );
}
