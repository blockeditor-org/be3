use super::*;

#[test]
fn a_message_hash_covers_what_it_signs_and_a_sent_message_notes_the_last_one_seen() {
    let chat = sent(&ChatContent::default(), ADA, &["hello", "there"]);
    let [hello, there] = [chat.messages()[0], chat.messages()[1]];
    let first = chat.message(hello).expect("the first message was sent");
    let second = chat.message(there).expect("the second message was sent");

    assert_eq!(first.seen, None, "the first message saw nothing before it");
    assert_eq!(second.seen, chat.hash(CHAT, hello));
    assert!(first.signature.0.is_empty(), "nothing signs messages yet");

    let hash = first.hash(CHAT, hello);
    assert_eq!(
        hash,
        crate::MessageHash::of(&first.signed_bytes(CHAT, hello))
    );
    let changed = [
        Message {
            body: "hullo".to_owned(),
            ..first.clone()
        }
        .hash(CHAT, hello),
        Message {
            author: BO,
            ..first.clone()
        }
        .hash(CHAT, hello),
        Message {
            time: 9,
            ..first.clone()
        }
        .hash(CHAT, hello),
        first.clone().replying_to(Some(there)).hash(CHAT, hello),
        first.hash(Uuid::from_u128(1), hello),
        first.hash(CHAT, there),
    ];
    assert!(
        changed.iter().all(|other| *other != hash),
        "every signed field changes the hash"
    );
    let signature = Message {
        signature: crate::Signature(vec![1, 2, 3]),
        ..first.clone()
    };
    assert_eq!(
        signature.hash(CHAT, hello),
        hash,
        "the signature is not part of what it signs"
    );
}
