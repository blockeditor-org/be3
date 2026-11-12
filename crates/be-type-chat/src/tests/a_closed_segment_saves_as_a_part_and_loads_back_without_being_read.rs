use be_block::Part;
use be_block_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};

use super::*;

fn reuses(part: &Part<'_>) -> usize {
    match part {
        Part::Reused(_) => 1,
        Part::Bytes(_) => 0,
        Part::Node { parts, .. } => parts.iter().map(reuses).sum(),
    }
}

#[test]
fn a_closed_segment_saves_as_a_part_and_loads_back_without_being_read() {
    let open = long(120);
    assert!(open.segments().is_empty());
    let chat = closed(&open);

    let segments = chat.segments();
    assert_eq!(
        segments.len(),
        1,
        "the oldest messages closed into one segment"
    );
    assert_eq!(
        chat.messages().len() as u64 + segments[0].count,
        120,
        "every message is recent or in the segment"
    );
    assert_eq!(
        chat.messages(),
        open.messages()[segments[0].count as usize..]
    );
    assert!(
        segments[0].length >= 64 * 1024 && segments[0].length <= 256 * 1024,
        "a segment is a part of 64 to 256 KiB: {}",
        segments[0].length
    );

    let Part::Node { parts, .. } = chat.parts() else {
        panic!("a chat saves as a node of parts");
    };
    assert_eq!(parts.len(), 2, "the header, then the segment");

    let keys = Keyring::new();
    keys.insert(CHAT, ContentKey::from_bytes([7; 32]));
    let vault = Vault::new(MemoryStore::new(), keys).with_chunker(ChunkerConfig::SMALL);
    let manifest = vault
        .write(CHAT, ChatContent::CONTENT_TYPE, chat.parts())
        .expect("the chat saves");
    let stored = vault.load_first(&manifest).expect("its header loads");
    assert!(
        (stored.bytes.len() as u64) < manifest.length(),
        "a replica that loads the first part holds only the header, not the {} bytes after it",
        segments[0].length
    );
    let loaded = ChatContent::from_stored(&stored).expect("a chat loads from its header");
    assert_eq!(loaded, chat);
    assert_eq!(loaded.segments(), segments);
    assert_eq!(
        reuses(&loaded.parts()),
        2,
        "a chat saved again reuses its header's parts and its segment, reading neither"
    );

    // The part is read by range, as an editor does, and is the segment the index describes.
    let ranges = loaded.segment_ranges();
    let bytes = vault
        .read_range(&manifest, ranges[0].start, ranges[0].end - ranges[0].start)
        .expect("the segment reads by range");
    let messages = loaded
        .read_segment(0, &bytes)
        .expect("the bytes are the segment's part");
    assert_eq!(messages.len() as u64, segments[0].count);
    assert_eq!(messages[0].0, open.messages()[0]);
    assert!(
        loaded.read_segment(0, &bytes[1..]).is_none(),
        "bytes that are not the part the index names are not read"
    );

    let again = vault
        .write(CHAT, ChatContent::CONTENT_TYPE, loaded.parts())
        .expect("the chat saves again");
    assert_eq!(again, manifest);
}
