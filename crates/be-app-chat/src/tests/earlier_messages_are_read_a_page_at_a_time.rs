use super::*;

/// A chat opens on its newest page of the log, and reads each earlier page as the person asks
/// for it.
#[test]
fn earlier_messages_are_read_a_page_at_a_time() {
    let messages: Vec<String> = (0..250).map(|index| format!("Message {index}")).collect();
    let said: Vec<(Uuid, &str)> = messages
        .iter()
        .map(|body| (ROSA, body.as_str()))
        .collect();
    // Opened again, the editor holds only what it reads.
    let chat = Harness::new(&said);
    let mut chat = chat.reopened();
    assert!(chat.editor.shown("chat.message.249"));
    assert!(chat.editor.shown("chat.message.150"));
    assert!(!chat.editor.shown("chat.message.149"));
    // Scrolled to the start, the chat offers what is before it.
    let top = |chat: &mut Harness, first: u64| {
        chat.editor
            .scroll(format!("chat.message.{first}"), vec2(0.0, -1_000_000.0));
        chat.run();
    };
    top(&mut chat, 150);
    assert!(chat.editor.shown("chat.older"));

    chat.click("chat.older");
    assert!(chat.editor.shown("chat.message.50"));
    top(&mut chat, 50);
    chat.click("chat.older");
    assert!(chat.editor.shown("chat.message.0"));
    top(&mut chat, 0);
    assert!(
        !chat.editor.shown("chat.older"),
        "nothing is earlier than the log's first entry"
    );
}
