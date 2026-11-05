use super::*;

#[test]
fn an_image_a_program_copies_reaches_the_host() {
    let mut harness = Harness::new();
    let control = harness.client.data_control();
    harness.settle();

    harness
        .client
        .copy_image_with_control(&control, "PNG bytes");
    let mut copied = None;
    for _ in 0..4 {
        harness.frame(Vec::new());
        copied = copied.or_else(|| harness.output.as_mut()?.copied.take());
    }

    let copied = copied.expect("the program's image is copied in the host");
    assert_eq!(
        copied.ready(beui::clipboard::formats::PNG),
        Some(&b"PNG bytes"[..]),
        "the host holds the image, so it outlives the program"
    );
    assert_eq!(copied.plain_text(), None);
}
