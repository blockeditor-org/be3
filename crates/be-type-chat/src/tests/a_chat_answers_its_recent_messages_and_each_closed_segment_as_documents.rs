use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use be_block_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};

use super::*;

#[test]
fn a_chat_answers_its_recent_messages_and_each_closed_segment_as_documents() {
    let filler = "a line of chat ".repeat(70);
    let bodies: Vec<String> = (0..120)
        .map(|index| format!("message {index} {filler}"))
        .collect();
    let bodies: Vec<&str> = bodies.iter().map(String::as_str).collect();
    let chat = closed(&sent(&ChatContent::default(), ADA, &bodies));
    let count = chat.segments()[0].count as usize;

    let keys = Keyring::new();
    keys.insert(CHAT, ContentKey::from_bytes([7; 32]));
    let vault = Rc::new(Vault::new(MemoryStore::new(), keys).with_chunker(ChunkerConfig::SMALL));
    let manifest = vault
        .write(CHAT, ChatContent::CONTENT_TYPE, chat.parts())
        .expect("saves");
    let stored = vault.load_first(&manifest).expect("loads the header");
    let loaded = ChatContent::from_stored(&stored).expect("loads");
    let reads = Rc::new(Cell::new(0));
    let reader = || {
        let (vault, reads) = (Rc::clone(&vault), Rc::clone(&reads));
        be_block_content::PartReader::new(move |part, offset, length| {
            reads.set(reads.get() + 1);
            let read = vault
                .read_part(part, offset, length)
                .map_err(|error| error.to_string());
            Box::pin(async move { read })
        })
    };
    let search = |chat: &ChatContent, keys: Option<&[&str]>, indexed: &[be_block_content::PartRef]| {
        let keys: Option<HashSet<String>> =
            keys.map(|keys| keys.iter().map(|key| (*key).to_owned()).collect());
        let indexed: HashSet<be_block_content::PartRef> = indexed.iter().copied().collect();
        pollster::block_on(chat.search_text(keys.as_ref(), &indexed, reader()))
            .expect("a chat answers its search text")
    };

    let documents = search(&loaded, None, &[]);
    assert_eq!(documents.len(), 2, "the recent messages, then the segment");
    assert_eq!(documents[0].key, "recent");
    assert_eq!(documents[0].source, None);
    assert_eq!(
        documents[0].body.as_deref(),
        Some(bodies[count..].join("\n").as_str())
    );
    let segment = documents[1].clone();
    let part = segment
        .source
        .expect("an untouched segment's source is its part");
    assert_eq!(segment.body, Some(bodies[..count].join("\n")));
    assert_eq!(reads.get(), 1, "the segment's part is read once");

    let documents = search(&loaded, None, &[part]);
    assert_eq!(documents[1].key, segment.key);
    assert_eq!(
        documents[1].body, None,
        "an indexed segment is not read again"
    );
    assert_eq!(reads.get(), 1);

    let documents = search(&loaded, Some(&["recent"]), &[]);
    assert_eq!(documents.len(), 1, "only the documents asked for");
    assert_eq!(reads.get(), 1);

    let oldest = chat
        .read_segment(0, &vault.read_part(part, 0, part.length()).unwrap())
        .expect("the segment reads")[0]
        .0;
    let edited = edited(&loaded, [loaded.edit(oldest, ADA, 200, "message 0, fixed")]);
    let documents = search(&edited, None, &[part]);
    assert_eq!(
        documents[1].key, segment.key,
        "a segment keeps its key when one of its messages is edited"
    );
    assert_eq!(
        documents[1].source, None,
        "a segment one of whose messages was edited is not taken from its part alone"
    );
    assert!(
        documents[1]
            .body
            .as_deref()
            .is_some_and(|body| body.starts_with("message 0, fixed\n")),
        "the edit is what the segment's text says"
    );
}
