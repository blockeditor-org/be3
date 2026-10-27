use super::*;
use beui_core::display::{Group, Layer};

#[test]
fn the_screen_reader_readout_sits_at_the_bottom_above_the_filters() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.drag_simulation_slider("blur", 1.0);
    let output = harness.frame(Vec::new());

    let filters = output.main().filters();
    let [filter] = filters.as_slice() else {
        panic!("the blur slider set {} filters", filters.len());
    };
    assert!(filter.blur > 0.0, "the blur slider read {}", filter.blur);
    let readout = harness.readout();
    assert!(readout.is_positive(), "the reader painted no readout");
    assert!(
        filter.region.bottom() <= readout.top(),
        "the filter reached {} into a readout starting at {}",
        filter.region.bottom(),
        readout.top()
    );

    let filtered = output
        .main()
        .layers
        .iter()
        .find_map(|layer| match layer {
            Layer::Group(Group::Filter(_), inner) => Some(beui_core::display::flatten(inner)),
            _ => None,
        })
        .expect("the filter covered no shapes");
    let highlight = |shape: &crate::native::Shape| {
        matches!(
            shape,
            crate::native::Shape::Rect { color, stroke_width, .. }
                if *stroke_width == 0.0 && color.to_array() == [82, 137, 255, 72]
        )
    };
    assert!(
        output.main().shapes().iter().any(highlight),
        "the reader painted no highlight"
    );
    assert!(
        filtered.iter().any(highlight),
        "the focus outline escaped the filter"
    );
}
