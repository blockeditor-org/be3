use be_block_content::{BlockContent, LiveEdit};
use be_block_model::BlockRef;

use super::*;
use crate::ChatContent;

/// A block made for a message not sent yet is the chat's child as a draft, and once the
/// message is sent, as what it carries; deleting the message takes it out.
#[test]
fn a_sent_message_lists_what_it_carries_until_it_is_deleted() {
    let block = Uuid::from_u128(0xb10c);
    let message = Uuid::from_u128(0x5e17);
    let mut chat = ChatContent::default();
    let drafted = chat.make_draft(block, ADA, 1);
    chat.apply(&drafted);
    assert_eq!(chat.drafts_of(ADA), [block]);
    assert_eq!(chat.references(), [block]);

    let sent = chat.send(message, ADA, 2, &[BlockRef(block)]).unwrap();
    chat.apply(&sent);
    assert!(chat.drafts_of(ADA).is_empty());
    assert_eq!(chat.references(), [block]);

    let deleted = chat.delete(message, ADA, 3).unwrap();
    chat.apply(&deleted);
    assert!(chat.references().is_empty());
    assert!(chat.delete(message, ADA, 4).is_none());
}
