use super::*;
use be_block_content::Root;
use be_type_chat::{ChatContent, ChatReads, ChatReadsContent, Message};

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

/// A chat with one message from `from`, sealed and sent, and its editor.
fn chat_with(from: Uuid, body: &str) -> (Uuid, Editor<ChatContent>) {
    let chat = Uuid::new_v4();
    create(
        chat,
        ChatContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named(body),
        Some(ChatContent::default().encode()),
        false,
    );
    flush_and_send();
    open(chat, ChatContent::CONTENT_TYPE);
    let mut editor = Editor::<ChatContent>::attach(chat);
    editor.wait("was sent the chat", |_| true);
    let (_, send) = editor.content().send(chat, Message::new(from, 1, body));
    editor.operate(send.into());
    editor.wait("showed the message", |chat| chat.messages().len() == 1);
    (chat, editor)
}

/// Each chat is counted with its own place in the account's reads: reading one chat moves only
/// its place, so only it is counted again, from the head already loaded, and the other chat is
/// neither loaded nor counted again.
#[test]
fn a_move_of_one_chats_read_place_recounts_that_chat_alone() {
    crate::unread::forget();
    let harness = Harness::start();
    harness.connect();
    let rosa = Uuid::new_v4();
    let (lunch, lunch_editor) = chat_with(rosa, "Who's in?");
    let (minutes, _minutes_editor) = chat_with(rosa, "Bring the minutes");
    flush();
    let reads = ChatReads::CONTENT_TYPE;
    crate::unread::publish(reads, Vec::new());
    counts(lunch, 1, "counted Rosa's message in the lunch chat");
    counts(minutes, 1, "counted Rosa's message in the minutes chat");
    flush();
    let before = with_shared(|shared| shared.counting.made).unwrap_or_default();

    let newest = lunch_editor
        .content()
        .newest()
        .expect("the lunch chat has a message");
    let mut read = ChatReadsContent::default();
    read.apply(&ChatReads::set_read(lunch, Uuid::new_v4(), newest, &[]));
    crate::unread::publish(reads, vec![read.encode()]);
    counts(
        lunch,
        0,
        "counted nothing in the lunch chat once it was read",
    );
    flush();
    let after = with_shared(|shared| shared.counting.made).unwrap_or_default();
    assert_eq!(
        after.0, before.0,
        "a move of a place alone loads no head again"
    );
    assert_eq!(
        after.1,
        before.1 + 1,
        "only the chat whose place moved is counted again"
    );
    counts(minutes, 1, "the minutes chat keeps its count");
    crate::unread::forget();
}
