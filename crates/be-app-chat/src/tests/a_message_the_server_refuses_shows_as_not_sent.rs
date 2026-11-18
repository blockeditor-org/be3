use super::*;

/// A message the server refuses shows as not sent, to send again or discard.
#[test]
fn a_message_the_server_refuses_shows_as_not_sent() {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    chat.editor.store().refuse_appends(Some("no access"));
    chat.send("I think so");
    let refused = "refused.0";
    assert!(chat.row(refused).contains("Not sent"));
    assert_eq!(chat.bodies(), ["Can you still see this chat?"]);
    chat.snapshot("a_message_the_server_refuses_shows_as_not_sent");

    chat.editor.store().refuse_appends(None);
    chat.click(&format!("chat.message.{refused}.retry"));
    assert!(!chat.editor.shown(&format!("chat.message.{refused}")));
    assert_eq!(chat.bodies(), ["Can you still see this chat?", "I think so"]);

    chat.editor.store().refuse_appends(Some("no access"));
    chat.send("Never mind");
    let refused = "refused.0";
    assert!(chat.row(refused).contains("Not sent"));
    chat.click(&format!("chat.message.{refused}.discard"));
    assert!(!chat.editor.shown(&format!("chat.message.{refused}")));
    assert_eq!(chat.bodies(), ["Can you still see this chat?", "I think so"]);
}
