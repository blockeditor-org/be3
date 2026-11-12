use super::*;

#[test]
fn a_small_message_before_an_oversized_one_does_not_stall_closing() {
    let mut chat = sent(&ChatContent::default(), ADA, &["small"]);
    let huge = "x".repeat(300 * 1024);
    chat = sent(&chat, ADA, &[huge.as_str()]);
    let rest = "a line of chat ".repeat(70);
    let rest: Vec<&str> = (0..200).map(|_| rest.as_str()).collect();
    chat = sent(&chat, ADA, &rest);
    let mut closes = 0;
    while let Some(closing) = chat.closing() {
        chat.apply(&closing);
        closes += 1;
        assert!(closes < 10);
    }
    assert!(
        closes >= 2,
        "the small message closes, then the large one alone"
    );
    assert!(chat.segments().len() >= 2);
}
