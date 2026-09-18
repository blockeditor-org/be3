use super::*;

#[test]
fn turning_on_the_screen_reader_covers_the_document_and_reads_what_it_is_on() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    let output = harness.frame(Vec::new());

    assert_eq!(harness.reading().as_deref(), Some("Hello, text"));
    assert!(
        harness
            .transcript()
            .iter()
            .any(|line| line == "Hello, text")
    );
    assert!(output.shapes().iter().any(|shape| matches!(
        shape,
        crate::Shape::Rect { rect, color, .. }
            if color.alpha() > 200 && color.to_array()[..3] == [0, 0, 0] && rect.width() > 400.0
    )));
}
