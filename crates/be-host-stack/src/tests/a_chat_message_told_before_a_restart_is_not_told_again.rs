use super::*;

#[test]
fn a_chat_message_told_before_a_restart_is_not_told_again() {
    let harness = Harness::pushing();
    let room = ChatRoom::start(&harness, &[]);
    let mut editor = room.editor();
    room.send(&mut editor, "Running late", None);
    room.told_away();
    drop(editor);
    stop();

    room.restart(&harness);
    wait_until("reconnected", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    room.told_nothing_more(&mut room.editor());
}
