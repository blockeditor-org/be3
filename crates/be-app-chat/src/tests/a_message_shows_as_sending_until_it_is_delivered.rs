use super::*;

#[test]
fn a_message_shows_as_sending_until_it_is_delivered() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);

    chat.editor.store().defer(true);
    chat.send("Just got on");
    assert_eq!(
        chat.sending().len(),
        1,
        "a message the host has not taken is sending"
    );
    chat.editor.store().defer(false);
    chat.run();
    let on = chat.id(1);
    assert!(chat.sending().is_empty());

    chat.editor.store().set_published(None, false);
    chat.run();
    chat.send("The signal keeps dropping");
    let offline = chat.id(2);
    assert!(
        chat.row(offline).contains("Sending"),
        "a message sent offline is sending until it is published"
    );
    assert!(
        !chat.row(on).contains("Sending"),
        "what was delivered before stays delivered"
    );
    chat.snapshot("a_message_sent_offline_shows_as_sending");

    chat.editor.store().set_published(None, true);
    chat.run();
    assert!(!chat.row(offline).contains("Sending"));
}
