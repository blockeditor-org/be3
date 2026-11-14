use super::*;

/// `message`, signed by its author, as the session owner sequenced it once the server took it
/// at `received`.
fn arrived(content: &mut ChatContent, message: Message, received: Option<i64>) -> ObjectId {
    let send = signed_by(content, message.author, message);
    let id = ChatContent::sent_in(&send.clone().into())[0];
    let mut operation = ChatOp::from(send);
    *ChatContent::arrival(&mut operation).expect("a send keeps its arrival") = received;
    content.apply(&operation);
    id
}

/// A message the server took well after its claimed time was written offline: it shows when it
/// arrived beside when its author says they wrote it. One that came at once, or whose arrival
/// is unknown (the owner's own), shows only its claimed time.
#[test]
fn a_message_sent_offline_shows_when_it_arrived() {
    let mut content = ChatContent::default();
    let asked = arrived(
        &mut content,
        Message::new(ME, MORNING, "Are the installers up yet?"),
        None,
    );
    let online = arrived(
        &mut content,
        Message::new(ROSA, MORNING + 2 * MINUTE, "Not yet, I'm boarding now."),
        Some(MORNING + 2 * MINUTE + 1_500),
    );
    let offline = arrived(
        &mut content,
        Message::new(
            ROSA,
            MORNING + 5 * MINUTE,
            "Written on the train: the Linux one is up.",
        ),
        Some(MORNING + 37 * MINUTE),
    );
    let mut chat = Harness::open(content, &[], CHAT, false);

    assert_eq!(chat.row(asked), "You 09:00 Are the installers up yet?");
    assert_eq!(
        chat.row(online),
        "Member 7e1a 09:02 Not yet, I'm boarding now."
    );
    assert_eq!(
        chat.row(offline),
        "Member 7e1a 09:05 sent offline, arrived 09:37 Written on the train: the Linux one is up."
    );
    chat.snapshot("a_message_sent_offline_shows_when_it_arrived");
}
