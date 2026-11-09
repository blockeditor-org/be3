use be_block::{Part, Stored};
use be_block_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};

use super::*;

fn reuses(part: &Part<'_>) -> bool {
    match part {
        Part::Reused(_) => true,
        Part::Bytes(_) => false,
        Part::Node { parts, .. } => parts.iter().any(reuses),
    }
}

#[test]
fn a_saved_chat_loads_back_from_its_parts() {
    let chat = sent(&ChatContent::default(), ADA, &["one", "two"]);
    let chat = edited(&chat, [chat.react(first(&chat), BO, "eye", 1)]);

    let Part::Node { parts, .. } = chat.parts() else {
        panic!("a chat saves as a node of parts");
    };
    assert_eq!(parts.len(), 1, "a chat saves only its header so far");
    assert_eq!(chat.parts().flatten(), Some(chat.encode()));
    let unsaved = ChatContent::from_stored(&Stored::unsaved(chat.encode()))
        .expect("a chat loads its unsaved stored form");
    assert_eq!(unsaved, chat);

    let keys = Keyring::new();
    keys.insert(CHAT, ContentKey::from_bytes([7; 32]));
    let vault = Vault::new(MemoryStore::new(), keys).with_chunker(ChunkerConfig::SMALL);
    let manifest = vault
        .write(CHAT, ChatContent::CONTENT_TYPE, chat.parts())
        .expect("the chat saves");
    let stored = vault.load_first(&manifest).expect("its header loads");
    let loaded = ChatContent::from_stored(&stored).expect("a chat loads from its header");

    assert_eq!(loaded, chat);
    assert!(
        reuses(&loaded.parts()),
        "a chat saved again unchanged reuses its header's parts"
    );
    let again = vault
        .write(CHAT, ChatContent::CONTENT_TYPE, loaded.parts())
        .expect("the chat saves again");
    assert_eq!(again, manifest);
    assert_eq!(round_trip(&chat), chat);
}
