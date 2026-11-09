use super::*;

#[test]
fn unread_counts_what_others_sent_after_the_last_message_read() {
    let chat = sent(&ChatContent::default(), ADA, &["One", "Two"]);
    assert_eq!(chat.unread(None, BO), 2, "with nothing read, all of it");
    let read = chat.newest().expect("the chat has messages");
    assert_eq!(chat.unread(Some(&read), BO), 0);

    let mut chat = sent(&chat, BO, &["Mine"]);
    let (_, later) = chat.send(CHAT, Message::new(ADA, 10, "Three"));
    chat.apply(&later);
    assert_eq!(
        chat.unread(Some(&read), BO),
        1,
        "my own messages are not unread"
    );

    let deleted = edited(&chat, [chat.delete(read.message, ADA, 11)]);
    assert_eq!(
        deleted.unread(Some(&read), BO),
        1,
        "once the message read is gone, its time says where reading stopped"
    );
}
