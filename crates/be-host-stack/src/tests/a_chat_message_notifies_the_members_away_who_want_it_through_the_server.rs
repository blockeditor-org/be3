use be_block_protocol::NotifyMode;

use super::*;

#[test]
fn a_chat_message_notifies_the_members_away_who_want_it_through_the_server() {
    let harness = Harness::pushing();
    let room = ChatRoom::start(
        &harness,
        &[
            ("present", None, true),
            ("off", Some(NotifyMode::Off), false),
            ("quiet", Some(NotifyMode::Mentions), false),
        ],
    );
    let mut editor = room.editor();

    // Only the member who is away and wants every message is told: not the one connected, not
    // the one who turned the chat off, not the one who wants only mentions.
    room.send(&mut editor, "Noon at the usual place", None);
    room.told_away("Noon at the usual place");

    // A reply mentions the author of the message it answers, so the member who wants only
    // mentions is told too, urgently. Each account's pushes go in order, so one about the
    // first message would have reached the connected or muted member before this one.
    room.send(&mut editor, "Are you coming?", Some(room.account("quiet")));
    let mut told = vec![room.next(), room.next()];
    told.sort();
    let asked = "Are you coming?".to_owned();
    assert_eq!(
        told,
        [
            ("away".to_owned(), "normal".to_owned(), asked.clone()),
            ("quiet".to_owned(), "high".to_owned(), asked),
        ]
    );
    room.told_nothing_more(&mut editor);
}
