use super::*;
use crate::selection::client;

#[test]
fn text_a_program_copies_reaches_the_host() {
    let mut harness = Harness::new();
    let control = harness.client.data_control();
    harness.settle();

    harness
        .client
        .copy_with_control(&control, "copied in a program");
    let mut copied = None;
    for _ in 0..4 {
        harness.frame(Vec::new());
        copied = copied.or_else(|| harness.output.as_mut()?.copied_text.take());
    }

    assert_eq!(copied.as_deref(), Some("copied in a program"));
    harness.settle();
    assert!(
        !client::source_cancelled(),
        "the program keeps the selection the host was handed"
    );
}
