use std::collections::HashSet;

use super::*;

/// One owner's part writer leaves a part unnamed; another owner's vault over the same store
/// finds the part's objects, and the first owner's collection, run on another thread while that
/// look is under way and before it claims what it found, removes nothing. Once the look is done
/// the objects are claimed, and no collection removes them. Removed, the second owner would have
/// been told the store holds what it no longer does.
#[test]
fn a_collection_removes_nothing_while_another_owners_access_is_under_way() {
    let store = MemoryStore::new();
    let unnamed = Unnamed::default();
    let first = Arc::new(
        Vault::new(store.clone(), keys(1))
            .with_chunker(ChunkerConfig::SMALL)
            .with_unnamed(unnamed.clone(), Hash::of(b"first")),
    );
    let interrupting = Interrupting {
        inner: store.clone(),
        during: Arc::default(),
    };
    let second = Vault::new(interrupting.clone(), keys(1))
        .with_chunker(ChunkerConfig::SMALL)
        .with_unnamed(unnamed, Hash::of(b"second"));

    let mut writer = first.part_writer(id(1)).unwrap();
    writer.write(&pseudorandom(8 * 1024, 5)).unwrap();
    let part = writer.finish().unwrap().part().expect("bytes were written");
    let objects = first
        .objects(&Manifest {
            content_type: CONTENT,
            root: Some(part),
        })
        .unwrap();
    assert_eq!(first.unnamed_count(), objects.len(), "let go of, unnamed");

    let removed = Arc::new(AtomicUsize::new(0));
    *interrupting.during.lock().unwrap() = Some(Box::new({
        let first = Arc::clone(&first);
        let removed = Arc::clone(&removed);
        move || {
            let first = Arc::clone(&first);
            let collected = std::thread::spawn(move || first.collect(|_| Ok(HashSet::new())))
                .join()
                .unwrap()
                .unwrap();
            removed.fetch_add(collected, Ordering::Relaxed);
        }
    }));
    for object in &objects {
        assert!(second.store().has(*object).unwrap(), "{object} found");
    }
    *interrupting.during.lock().unwrap() = None;
    assert_eq!(
        removed.load(Ordering::Relaxed),
        0,
        "nothing removed mid-look"
    );

    assert_eq!(first.unnamed_count(), 0, "claimed once found");
    assert_eq!(first.collect(|_| Ok(HashSet::new())).unwrap(), 0);
    for object in &objects {
        assert!(store.has(*object).unwrap(), "{object} stays");
    }
}
