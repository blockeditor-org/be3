use super::*;

#[test]
fn the_screen_reader_curtain_paints_above_the_filters_that_blur_the_document() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.drag_simulation_slider("blur", 1.0);
    let output = harness.frame(Vec::new());

    let filter = output.filter().expect("the blur slider set no filter");
    assert!(filter.blur > 0.0, "the blur slider read {}", filter.blur);
    let boundary = output
        .filtered_shapes()
        .expect("the filter covered no shapes");

    let highlight = output
        .shapes()
        .iter()
        .position(|shape| {
            matches!(
                shape,
                crate::Shape::Rect { color, stroke_width, .. }
                    if *stroke_width == 0.0 && color.to_array()[..3] == [82, 137, 255]
            )
        })
        .expect("the reader painted no highlight");
    let curtain = output
        .shapes()
        .iter()
        .position(|shape| {
            matches!(
                shape,
                crate::Shape::Rect { rect, color, .. }
                    if color.alpha() > 200
                        && color.to_array()[..3] == [0, 0, 0]
                        && rect.width() > 400.0
            )
        })
        .expect("the reader painted no curtain");

    assert!(highlight < boundary, "the focus outline escaped the filter");
    assert!(
        curtain >= boundary,
        "the curtain was filtered with the document"
    );
}
