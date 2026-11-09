use std::time::Duration;

use super::*;

#[test]
fn a_message_shows_as_sending_until_it_is_delivered() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);

    chat.write("Just got on");
    chat.click("chat.send");
    let on = chat.id(1);
    assert!(
        chat.row(on).contains("Sending"),
        "a message its replica confirmed is sending until the server holds it"
    );
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    assert!(
        !chat.row(on).contains("Sending"),
        "the server took the journal entry"
    );

    chat.editor.store().set_online(false);
    chat.write("The signal keeps dropping");
    chat.click("chat.send");
    let offline = chat.id(2);
    chat.editor.advance(Duration::from_secs(40));
    chat.run();
    assert!(
        chat.row(offline).contains("Sending"),
        "a message sent offline is sending, sealed or not, until it is published"
    );
    assert!(
        !chat.row(on).contains("Sending"),
        "what was delivered before stays delivered"
    );
    chat.snapshot("a_message_sent_offline_shows_as_sending");

    chat.editor.store().set_online(true);
    chat.run();
    assert!(!chat.row(offline).contains("Sending"));
}
