use super::*;

/// A chat opens on its newest page of the log, and reads each earlier page as the person asks
/// for it, keeping every page read back, past the editor's `LOG_HELD` too.
#[test]
fn earlier_messages_are_read_a_page_at_a_time() {
    // More than an editor holds unless it pages back (`editor::LOG_HELD`, 1024).
    let count = 1100;
    let messages: Vec<String> = (0..count).map(|index| format!("Message {index}")).collect();
    let said: Vec<(Uuid, &str)> = messages.iter().map(|body| (ROSA, body.as_str())).collect();
    // Opened again, the editor holds only what it reads.
    let chat = Harness::new(&said);
    let mut chat = chat.reopened();
    let newest = count - 1;
    let mut first = count - 100;
    assert!(chat.editor.shown(&format!("chat.message.{newest}")));
    assert!(chat.editor.shown(&format!("chat.message.{first}")));
    assert!(!chat.editor.shown(&format!("chat.message.{}", first - 1)));
    // Scrolled to the start, the chat offers what is before it.
    let top = |chat: &mut Harness, first: u64| {
        chat.editor
            .scroll(format!("chat.message.{first}"), vec2(0.0, -1_000_000.0));
        chat.run();
    };
    while first > 0 {
        top(&mut chat, first);
        assert!(chat.editor.shown("chat.older"));
        chat.click("chat.older");
        first = first.saturating_sub(100);
        assert!(
            chat.editor.shown(&format!("chat.message.{first}")),
            "the page from {first} is read and kept"
        );
    }
    top(&mut chat, 0);
    assert!(
        !chat.editor.shown("chat.older"),
        "nothing is earlier than the log's first entry"
    );
    assert!(
        chat.editor.shown(&format!("chat.message.{newest}")),
        "the newest message is still held below"
    );
}
