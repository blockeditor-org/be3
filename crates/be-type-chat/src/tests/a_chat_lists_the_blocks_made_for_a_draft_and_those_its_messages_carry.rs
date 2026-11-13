use be_block::ChildChange;
use be_block_model::BlockRef;

use super::*;

#[test]
fn a_chat_lists_the_blocks_made_for_a_draft_and_those_its_messages_carry() {
    let (sketch, photo, kept) = (
        Uuid::from_u128(0x5c7c),
        Uuid::from_u128(0x9407),
        Uuid::from_u128(0x6e97),
    );
    let mut chat = long(0);
    for (block, author) in [(sketch, ADA), (photo, ADA), (kept, BO)] {
        let draft = chat.make_draft(block, author, 1);
        chat.apply(&draft.into());
    }
    assert_eq!(chat.drafts_of(ADA), [sketch, photo]);
    for block in [sketch, photo, kept] {
        assert!(
            chat.references().contains(&block),
            "a block made for a draft is the chat's child before it is sent"
        );
    }

    // Deleting a draft as a child unlists it.
    let deleting = chat
        .child_operations(ChildChange::Delete(photo))
        .expect("a draft can be deleted");
    for operation in &deleting {
        chat.apply(operation);
    }
    assert_eq!(chat.drafts_of(ADA), [sketch]);
    assert!(!chat.references().contains(&photo));

    // Sending takes what the message carries out of the drafts, and the message lists it.
    let message = Message {
        attachments: vec![BlockRef(sketch)],
        ..Message::new(ADA, 2, "The sketch")
    };
    let (id, message) = chat.draft(CHAT, message);
    let send = chat.send_drafted(id, &message);
    chat.apply(&send.into());
    assert!(chat.drafts_of(ADA).is_empty());
    assert_eq!(chat.drafts_of(BO), [kept], "someone else's draft stays theirs");
    assert!(chat.references().contains(&sketch));
    assert!(
        chat.child_operations(ChildChange::Delete(sketch)).is_none(),
        "a block a message carries is not taken out from under it"
    );

    // Once the message is closed into a segment the chat still lists what it carries.
    let body = "a line of chat ".repeat(70);
    let chat = closed(&sent(&chat, ADA, &vec![body.as_str(); 120]));
    assert!(
        chat.segments()
            .iter()
            .any(|segment| segment.attachments.contains(&BlockRef(sketch)))
    );
    assert!(
        chat.message(id).is_none(),
        "the message left the header"
    );
    assert!(chat.references().contains(&sketch));
}
