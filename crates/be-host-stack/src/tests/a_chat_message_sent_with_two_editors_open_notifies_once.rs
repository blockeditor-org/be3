use super::*;

#[test]
fn a_chat_message_sent_with_two_editors_open_notifies_once() {
    let harness = Harness::pushing();
    let room = ChatRoom::start(&harness, &[]);
    let mut editor = room.editor();
    let mut beside = room.editor();
    room.send(&mut editor, "Running late", None);
    beside.until("showed the message", |beside| {
        beside.content().messages().len() == 2
    });
    room.told_away("Running late");
    room.told_nothing_more(&mut beside);
}
