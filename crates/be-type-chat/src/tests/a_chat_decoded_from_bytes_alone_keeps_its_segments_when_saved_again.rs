use be_block_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};

use super::*;

fn empty_leaves(part: &be_block::Part<'_>) -> usize {
    match part {
        be_block::Part::Bytes(bytes) => usize::from(bytes.is_empty()),
        be_block::Part::Reused(_) => 0,
        be_block::Part::Node { parts, .. } => parts.iter().map(empty_leaves).sum(),
    }
}

#[test]
fn a_chat_decoded_from_bytes_alone_keeps_its_segments_when_saved_again() {
    let chat = closed(&long(120));
    let view = round_trip(&chat);
    assert_eq!(view.segments(), chat.segments());
    let keys = Keyring::new();
    keys.insert(CHAT, ContentKey::from_bytes([7; 32]));
    let vault = Vault::new(MemoryStore::new(), keys).with_chunker(ChunkerConfig::SMALL);
    let manifest = vault
        .write(CHAT, ChatContent::CONTENT_TYPE, view.parts())
        .expect("saves");
    let stored = vault.load_first(&manifest).expect("loads");
    let loaded = ChatContent::from_stored(&stored).expect("loads");
    assert_eq!(loaded.segments().len(), 1);
    assert_eq!(
        loaded.segment_ranges().len(),
        1,
        "a segment with no part stays an entry"
    );
    assert_eq!(
        empty_leaves(&view.parts()),
        0,
        "no placeholder part is saved for a segment the replica lacks"
    );
}
