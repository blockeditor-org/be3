use super::*;
use be_block_content::Root;
use be_type_chat::{ChatReads, ChatReadsContent};

/// Waits until the chat's node says `count` are unread.
fn counts(chat: Uuid, count: u32, what: &str) {
    crate::wait_until(what, |shared| {
        shared
            .graph
            .get(chat)
            .filter(|node| node.unread == count)
            .map(|_| ())
    });
}

/// Each chat is counted with its own place in the account's reads: reading one chat moves only
/// its place, so only it is loaded and counted again, and the other chat is not.
#[test]
fn a_move_of_one_chats_read_place_recounts_that_chat_alone() {
    crate::unread::forget();
    let harness = Harness::pushing();
    let room = ChatRoom::start(&harness, &[("rosa", None, true)]);
    let lunch = room.another("Lunch", "rosa", "Who's in?");
    let minutes = room.another("Minutes", "rosa", "Bring the minutes");
    flush();
    let reads = ChatReads::CONTENT_TYPE;
    crate::unread::publish(reads, Vec::new());
    counts(lunch, 1, "counted Rosa's message in the lunch chat");
    counts(minutes, 1, "counted Rosa's message in the minutes chat");
    flush();
    let before = with_shared(|shared| shared.counting.made).unwrap_or_default();

    let mut read = ChatReadsContent::default();
    read.apply(&ChatReads::set_read(lunch, Uuid::new_v4(), 0));
    crate::unread::publish(reads, vec![read.encode()]);
    counts(
        lunch,
        0,
        "counted nothing in the lunch chat once it was read",
    );
    flush();
    let after = with_shared(|shared| shared.counting.made).unwrap_or_default();
    assert_eq!(
        (after.0, after.1),
        (before.0 + 1, before.1 + 1),
        "only the chat whose place moved is loaded and counted again"
    );
    counts(minutes, 1, "the minutes chat keeps its count");
    crate::unread::forget();
}
