use super::*;

/// A message waits on the device until the server takes it, and shows as sending until then,
/// in a chat opened again too.
#[test]
fn a_message_shows_as_sending_until_it_is_delivered() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);

    chat.editor.store().set_online(false);
    chat.write("The signal keeps dropping");
    chat.click("chat.send");
    let kept = chat.kept(0);
    assert!(
        chat.row(&kept).contains("Sending"),
        "a message is sending until the server takes it"
    );
    chat.snapshot("a_message_sent_offline_shows_as_sending");

    let mut again = chat.reopened();
    assert!(
        again.row(&kept).contains("Sending"),
        "a chat opened again still shows what the server lacks as sending"
    );

    again.editor.store().set_online(true);
    again.run();
    let sent = again.seq(1);
    assert!(!again.editor.shown(&format!("chat.message.{kept}")));
    assert!(!again.row(sent).contains("Sending"));
    assert_eq!(again.bodies()[1], "The signal keeps dropping");
}
