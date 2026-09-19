use super::*;

#[test]
fn turning_on_the_screen_reader_reads_what_it_is_on() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    let output = harness.frame(Vec::new());

    assert_eq!(harness.reading().as_deref(), Some("Hello, text"));
    assert_eq!(harness.spoken().as_deref(), Some("Hello, text"));

    let readout = output
        .shapes()
        .iter()
        .find_map(|shape| match shape {
            crate::Shape::Rect { rect, color, .. } if color.to_array() == [14, 17, 23, 232] => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("the reader painted no readout");
    assert!(
        readout.bottom() >= TALL_VIEWPORT.y,
        "the readout stopped at {}",
        readout.bottom()
    );
    assert!(
        readout.height() < TALL_VIEWPORT.y / 4.0,
        "the readout took {} of the viewport",
        readout.height()
    );
    assert!(
        readout.width() > TALL_VIEWPORT.x / 2.0,
        "the readout was only {} wide",
        readout.width()
    );
}
