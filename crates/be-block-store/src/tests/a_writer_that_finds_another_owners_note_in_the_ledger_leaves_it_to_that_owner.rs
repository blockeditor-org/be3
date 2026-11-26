use std::collections::{HashMap, HashSet};

use super::*;

/// A ledger in memory, as be-block-client's keeps its notes: the first note stays.
#[derive(Default)]
struct Kept(Mutex<HashMap<Hash, Noted>>);

impl Ledger for Kept {
    fn noted(&self) -> Result<Vec<(Hash, Noted)>, StoreError> {
        Ok(self.0.lock().unwrap().iter().map(|(k, v)| (*k, *v)).collect())
    }

    fn note(&self, object: Hash, noted: Noted) -> Result<Option<Noted>, StoreError> {
        let mut kept = self.0.lock().unwrap();
        if let Some(first) = kept.get(&object) {
            return Ok(Some(*first));
        }
        kept.insert(object, noted);
        Ok(None)
    }

    fn forget(&self, objects: &[Hash]) -> Result<(), StoreError> {
        let mut kept = self.0.lock().unwrap();
        for object in objects {
            kept.remove(object);
        }
        Ok(())
    }
}

/// One owner's part writer has noted a part's objects in the ledger and not yet in the notes
/// every vault shares when another owner's writer seals the same bytes. That one never takes
/// the note over but forgets it, so a crash leaves the objects noted as neither owner's, and a
/// collection after the restart removes nothing the second's commit may name; and once the
/// first writer reaches the shared notes it finds the second's and claims them, so neither
/// owner's collection removes what both may name.
#[test]
fn a_writer_that_finds_another_owners_note_in_the_ledger_leaves_it_to_that_owner() {
    let store = MemoryStore::new();
    let ledger = Arc::new(Kept::default());
    let unnamed = Unnamed::with_ledger(ledger.clone()).unwrap();
    let vault = |owner: &[u8]| {
        Vault::new(store.clone(), keys(1))
            .with_chunker(ChunkerConfig::SMALL)
            .with_unnamed(unnamed.clone(), Hash::of(owner))
    };
    let (first, second) = (vault(b"first"), vault(b"second"));
    let bytes = pseudorandom(8 * 1024, 11);
    let objects = first
        .objects(&first.write(id(1), CONTENT, bytes.as_slice()).unwrap())
        .unwrap();
    for object in &objects {
        store.remove(*object).unwrap();
        ledger
            .note(
                *object,
                Noted {
                    block: Uuid::nil(),
                    owner: Hash::of(b"first"),
                },
            )
            .unwrap();
    }

    let mut writer = second.part_writer(id(1)).unwrap();
    writer.write(&bytes).unwrap();
    let held = writer.finish().unwrap();
    assert!(
        ledger
            .noted()
            .unwrap()
            .iter()
            .all(|(_, noted)| noted.owner != Hash::of(b"second")),
        "never noted as the second owner's in the ledger"
    );
    let restarted = Vault::new(store.clone(), keys(1))
        .with_unnamed(Unnamed::with_ledger(ledger.clone()).unwrap(), Hash::of(b"first"));
    assert_eq!(
        restarted.collect(|_| Ok(HashSet::new())).unwrap(),
        0,
        "nothing noted after a crash"
    );

    let mut writer = first.part_writer(id(1)).unwrap();
    writer.write(&bytes).unwrap();
    drop(writer.finish().unwrap());
    drop(held);
    assert_eq!(first.collect(|_| Ok(HashSet::new())).unwrap(), 0);
    assert_eq!(second.collect(|_| Ok(HashSet::new())).unwrap(), 0);
    for object in &objects {
        assert!(store.has(*object).unwrap(), "{object} stays");
    }
}
