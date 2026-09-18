use super::*;

#[test]
fn the_screen_reader_highlight_goes_under_the_curtain_that_hides_it() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    let output = harness.frame(Vec::new());

    let position = |wanted: [u8; 3]| {
        output.shapes().iter().position(|shape| {
            matches!(
                shape,
                crate::Shape::Rect { color, stroke_width, .. }
                    if *stroke_width == 0.0 && color.to_array()[..3] == wanted
            )
        })
    };
    let highlight = position([82, 137, 255]).expect("the reader painted no highlight");
    let curtain = position([0, 0, 0]).expect("the reader painted no curtain");

    assert!(highlight < curtain);
}
