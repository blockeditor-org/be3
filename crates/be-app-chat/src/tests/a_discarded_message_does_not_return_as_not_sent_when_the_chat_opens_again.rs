use super::*;

#[test]
fn a_discarded_message_does_not_return_as_not_sent_when_the_chat_opens_again() {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    chat.editor.store().set_online(false);
    chat.write("I think so");
    chat.click("chat.send");
    let refused = chat.id(1);
    chat.editor.advance(Duration::from_secs(40));
    chat.editor.store().refuse_kept();
    chat.run();
    assert!(chat.row(refused).contains("Not sent"));

    chat.click(&format!("chat.message.{refused}.discard"));
    chat.editor.advance(Duration::from_millis(500));
    assert_eq!(chat.bodies(), ["Can you still see this chat?"]);

    chat.editor.store().close_replica(None);
    let again = chat.reopened();
    assert_eq!(
        again.bodies(),
        ["Can you still see this chat?"],
        "the discarded message stays gone"
    );
    assert!(
        !again.row(again.id(0)).contains("Not sent"),
        "the refused commit this device still holds does not bring the failure back"
    );
}
