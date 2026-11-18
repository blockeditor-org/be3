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

/// A chat's unread count is counted from its log's latest entries and the account's reads, and
/// counted again as its log grows.
#[test]
fn a_chats_unread_count_is_counted_from_its_head_and_the_accounts_reads() {
    crate::unread::forget();
    let harness = Harness::pushing();
    let room = ChatRoom::start(&harness, &[("rosa", None, true)]);
    flush();
    let reads = ChatReads::CONTENT_TYPE;
    crate::unread::publish(reads, Vec::new());
    counts(
        room.block,
        2,
        "counted both members' messages, with nothing read",
    );
    assert!(
        crate::unread::kinds().contains(&reads),
        "the chat's module says which kind it counts from"
    );

    let mut read = ChatReadsContent::default();
    read.apply(&ChatReads::set_read(room.block, Uuid::new_v4(), 1));
    crate::unread::publish(reads, vec![read.encode()]);
    counts(
        room.block,
        0,
        "counted nothing once another device read up to the newest",
    );

    room.member("rosa").says(room.block, "Bring the minutes");
    counts(
        room.block,
        1,
        "counted the message that came after the place read",
    );
    crate::unread::forget();
}
