use super::*;
use be_block_content::Root;
use be_type_chat::{ChatContent, ChatReads, ChatReadsContent, Message};

/// Waits until the chat's node, at a head holding every message, says `count` are unread.
fn counts(chat: Uuid, count: u32, what: &str) {
    crate::wait_until(what, |shared| {
        shared
            .graph
            .get(chat)
            .filter(|node| node.unread == count)
            .map(|_| ())
    });
}

#[test]
fn a_chats_unread_count_is_counted_from_its_head_and_the_accounts_reads() {
    crate::unread::forget();
    let harness = Harness::start();
    harness.connect();
    let chat = Uuid::new_v4();
    create(
        chat,
        ChatContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Lunch"),
        Some(ChatContent::default().encode()),
        false,
    );
    flush_and_send();
    open(chat, ChatContent::CONTENT_TYPE);
    let mut editor = Editor::<ChatContent>::attach(chat);
    editor.wait("was sent the chat", |_| true);
    let rosa = Uuid::new_v4();
    for (at, body) in [(1, "Who's in?"), (2, "Noon at the usual place")] {
        let (_, send) = editor.content().send(chat, Message::new(rosa, at, body));
        editor.operate(send.into());
    }
    editor.wait("showed both messages", |chat| chat.messages().len() == 2);
    flush();
    let reads = ChatReads::CONTENT_TYPE;
    assert!(
        crate::unread::kinds().contains(&reads),
        "the chat's module says which kind it counts from"
    );

    crate::unread::publish(reads, Vec::new());
    counts(
        chat,
        2,
        "counted both of Rosa's messages, with nothing read",
    );

    let newest = editor.content().newest().expect("the chat has messages");
    let mut read = ChatReadsContent::default();
    read.apply(&ChatReads::set_read(chat, Uuid::new_v4(), newest, &[]));
    crate::unread::publish(reads, vec![read.encode()]);
    counts(
        chat,
        0,
        "counted nothing once another device read up to the newest",
    );

    let (_, answer) = editor
        .content()
        .send(chat, Message::new(rosa, 3, "Bring the minutes"));
    editor.operate(answer.into());
    flush();
    counts(
        chat,
        1,
        "counted the message that came after the place read",
    );
    crate::unread::forget();
}
