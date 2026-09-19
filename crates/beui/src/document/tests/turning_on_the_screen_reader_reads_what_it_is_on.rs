use super::*;

#[test]
fn turning_on_the_screen_reader_reads_what_it_is_on() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());

    assert_eq!(harness.reading().as_deref(), Some("Hello, text"));
    assert_eq!(harness.spoken().as_deref(), Some("Hello, text"));

    let readout = harness.readout();
    assert!(readout.is_positive(), "the reader painted no readout");
    assert_eq!(readout.bottom(), TALL_VIEWPORT.y);
    assert!(
        readout.width() > TALL_VIEWPORT.x / 2.0,
        "the readout was only {} wide",
        readout.width()
    );
    let bottom = harness.document_bottom();
    assert!(
        bottom <= readout.top(),
        "the document reached {bottom} into a readout starting at {}",
        readout.top()
    );
}
