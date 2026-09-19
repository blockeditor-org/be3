use super::*;

#[test]
fn the_screen_reader_readout_sits_at_the_bottom_above_the_filters() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.drag_simulation_slider("blur", 1.0);
    let output = harness.frame(Vec::new());

    let filter = output.filter().expect("the blur slider set no filter");
    assert!(filter.blur > 0.0, "the blur slider read {}", filter.blur);
    let boundary = output
        .filtered_shapes()
        .expect("the filter covered no shapes");

    let position = |wanted: [u8; 4]| {
        output.shapes().iter().position(|shape| {
            matches!(
                shape,
                crate::Shape::Rect { color, stroke_width, .. }
                    if *stroke_width == 0.0 && color.to_array() == wanted
            )
        })
    };
    let highlight = position([82, 137, 255, 72]).expect("the reader painted no highlight");
    let readout = position([14, 17, 23, 232]).expect("the reader painted no readout");

    assert!(highlight < boundary, "the focus outline escaped the filter");
    assert!(
        readout >= boundary,
        "the readout was filtered with the document"
    );
}
