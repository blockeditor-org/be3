use std::rc::Rc;

use be_block_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};

use super::*;

/// What the replica's resume merge does: merge the sides as `from_stored` loaded them, reading
/// the parts it settles from where they are saved.
#[test]
fn merging_saved_sides_with_differing_closes_keeps_each_segments_own_part() {
    let base = long(120);
    let ours = closed(&base);
    let mut other = base.clone();
    let deleted = base.messages()[3];
    other.apply(&base.delete(deleted, ADA, 9).into());
    let theirs = closed(&other);

    let keys = Keyring::new();
    keys.insert(CHAT, ContentKey::from_bytes([7; 32]));
    let vault = Rc::new(Vault::new(MemoryStore::new(), keys).with_chunker(ChunkerConfig::SMALL));
    let load = |chat: &ChatContent| {
        let manifest = vault
            .write(CHAT, ChatContent::CONTENT_TYPE, chat.parts())
            .expect("saves");
        let stored = vault.load_first(&manifest).expect("loads the header");
        (ChatContent::from_stored(&stored).expect("loads"), manifest)
    };
    let (base_loaded, _) = load(&base);
    let (ours_loaded, _) = load(&ours);
    let (theirs_loaded, _) = load(&theirs);

    let reader = {
        let vault = Rc::clone(&vault);
        be_block::PartReader::new(move |part, offset, length| {
            let read = vault
                .read_part(part, offset, length)
                .map_err(|error| error.to_string());
            Box::pin(async move { read })
        })
    };
    let (merged, conflicts) =
        merged_by_parts(&base_loaded, &ours_loaded, &theirs_loaded, reader).expect("merges");
    assert_eq!(conflicts, 0);
    assert_eq!(
        merged.segments().len(),
        1,
        "ours stand, and theirs, which overlapped them, went"
    );
    let (merged, manifest) = load(&merged);
    let mut closed = std::collections::HashSet::new();
    for (index, range) in merged.segment_ranges().into_iter().enumerate() {
        let bytes = vault
            .read_range(&manifest, range.start, range.end - range.start)
            .expect("the segment reads by range");
        let messages = merged
            .read_segment(index, &bytes)
            .expect("each entry has its own part, not the other's");
        closed.extend(messages.into_iter().map(|(id, _)| id));
    }
    let recent: Vec<_> = merged.messages();
    let recent_set: std::collections::HashSet<_> = recent.iter().copied().collect();
    assert_eq!(recent.len(), recent_set.len(), "no recent message twice");
    assert!(
        recent_set.is_disjoint(&closed),
        "a message is recent or closed"
    );
    let mut seen = recent_set;
    seen.extend(closed);
    seen.remove(&deleted);
    let mut expected: std::collections::HashSet<_> = base.messages().into_iter().collect();
    expected.remove(&deleted);
    assert_eq!(seen, expected, "no message lost");
}
