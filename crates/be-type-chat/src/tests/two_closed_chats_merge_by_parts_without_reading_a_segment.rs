use be_block_content::{Part, Stored};
use be_block_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};

use super::*;

#[test]
fn two_closed_chats_merge_by_parts_without_reading_a_segment() {
    let base = closed(&long(120));
    let keys = Keyring::new();
    keys.insert(CHAT, ContentKey::from_bytes([7; 32]));
    let vault = Vault::new(MemoryStore::new(), keys).with_chunker(ChunkerConfig::SMALL);
    let load = |chat: &ChatContent| -> ChatContent {
        let manifest = vault
            .write(CHAT, ChatContent::CONTENT_TYPE, chat.parts())
            .expect("the chat saves");
        let stored: Stored = vault.load_first(&manifest).expect("its header loads");
        ChatContent::from_stored(&stored).expect("a chat loads from its header")
    };
    let base = load(&base);
    let ours = load(&sent(&base, ADA, &["ours"]));
    let theirs = load(&sent(&base, BO, &["theirs"]));

    let (chat, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    assert_eq!(chat.segments().len(), 1);
    let Part::Node { parts, .. } = chat.parts() else {
        panic!("a chat saves as a node of parts");
    };
    assert!(
        matches!(parts.last(), Some(Part::Reused(_))),
        "the merged chat reuses the segment a side saved, so the merge reads none"
    );
}
