use super::*;

#[test]
fn unread_counts_from_the_furthest_message_read_by_order_not_by_claimed_time() {
    let chat = sent(&ChatContent::default(), ADA, &["One", "Two"]);
    assert_eq!(chat.unread(std::iter::empty(), BO), 2, "with nothing read, all of it");
    let two = chat.newest().expect("the chat has messages");
    assert_eq!(chat.unread([two], BO), 0);

    // Bo's clock runs an hour fast, Ada's a day slow: order decides, not their claims.
    let mut chat = chat;
    let (_, mine) = chat.send(CHAT, Message::new(BO, 3_600_000, "Mine"));
    chat.apply(&mine);
    let (_, three) = chat.send(CHAT, Message::new(ADA, -86_400_000, "Three"));
    chat.apply(&three);
    assert_eq!(chat.unread([two], BO), 1, "my own messages are not unread");

    let one = first(&chat);
    assert_eq!(
        chat.unread([one, two], BO),
        1,
        "the furthest of the devices' places counts"
    );
    assert!(chat.reads_further(two, Some(one)));
    assert!(
        !chat.reads_further(one, Some(two)),
        "a place never moves back"
    );

    let deleted = edited(&chat, [chat.delete(two, ADA, 11)]);
    assert_eq!(
        deleted.unread([two], BO),
        1,
        "once the message read is gone, the one before it marks the place"
    );
    let missing = ObjectId::new();
    assert_eq!(
        chat.unread([missing], BO),
        0,
        "a place this device has not received yet is past everything it has"
    );
}
