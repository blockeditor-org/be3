use std::collections::HashSet;

use super::*;

/// One owner's part writer leaves a part unnamed, and its collection's walk (`named`) runs
/// while the part is reached again: another owner's vault over the same store finds its
/// objects, which claims them, or the owner's own part writer writes the same bytes, which pins
/// them. Neither access is under way once the walk ends, so only the collection's look again
/// under the lock keeps it from removing what the walk took for unnamed.
#[test]
fn a_collection_passes_over_what_was_claimed_or_pinned_while_it_walked() {
    let store = MemoryStore::new();
    let unnamed = Unnamed::default();
    let first = Vault::new(store.clone(), keys(1))
        .with_chunker(ChunkerConfig::SMALL)
        .with_unnamed(unnamed.clone(), Hash::of(b"first"));
    let second = Vault::new(store.clone(), keys(1))
        .with_chunker(ChunkerConfig::SMALL)
        .with_unnamed(unnamed, Hash::of(b"second"));
    let bytes = pseudorandom(8 * 1024, 7);
    let let_go = || {
        let mut writer = first.part_writer(id(1)).unwrap();
        writer.write(&bytes).unwrap();
        let part = writer.finish().unwrap().part().expect("bytes were written");
        first
            .objects(&Manifest {
                content_type: CONTENT,
                root: Some(part),
            })
            .unwrap()
    };

    let objects = let_go();
    assert_eq!(first.unnamed_count(), objects.len(), "let go of, unnamed");
    let removed = first
        .collect(|_| {
            for object in &objects {
                assert!(second.store().has(*object)?, "{object} found");
            }
            Ok(HashSet::new())
        })
        .unwrap();
    assert_eq!(removed, 0, "claimed during the walk");
    for object in &objects {
        assert!(store.has(*object).unwrap(), "{object} stays");
    }

    let other = pseudorandom(8 * 1024, 8);
    let mut writer = first.part_writer(id(1)).unwrap();
    writer.write(&other).unwrap();
    let part = writer.finish().unwrap().part().expect("bytes were written");
    let objects = first
        .objects(&Manifest {
            content_type: CONTENT,
            root: Some(part),
        })
        .unwrap();
    assert_eq!(first.unnamed_count(), objects.len(), "let go of, unnamed");
    let mut held = None;
    let removed = first
        .collect(|_| {
            let mut writer = first.part_writer(id(1))?;
            writer.write(&other)?;
            held = Some(writer.finish()?);
            Ok(HashSet::new())
        })
        .unwrap();
    assert_eq!(removed, 0, "pinned during the walk");
    for object in &objects {
        assert!(store.has(*object).unwrap(), "{object} stays");
    }
    assert_eq!(
        first.unnamed_count(),
        objects.len(),
        "still noted while held"
    );
    drop(held);
    assert_eq!(
        first.collect(|_| Ok(HashSet::new())).unwrap(),
        objects.len()
    );
}
