use std::{cell::RefCell, collections::HashMap};

use super::*;
use be_block_store::{ContentKey, Keyring, MemoryStore, PartRef, Vault};
use be_lib_search::{Error, MemoryParts, PartKind, PartName, PartsOut};

use crate::{AsyncRead, MERGES_PER_PASS, REMEMBERED, SavedParts};

/// Saves each part to a vault once it is finished.
struct Saving<'a> {
    vault: &'a Vault<MemoryStore>,
    key: uuid::Uuid,
    writing: RefCell<HashMap<PartName, Vec<u8>>>,
    saved: RefCell<HashMap<PartName, Option<PartRef>>>,
}

impl PartsOut for Saving<'_> {
    fn write(&self, part: &PartName, bytes: &[u8]) -> Result<(), Error> {
        let mut writing = self.writing.borrow_mut();
        writing.entry(*part).or_default().extend_from_slice(bytes);
        Ok(())
    }

    fn finish(&self, part: &PartName) -> Result<(), Error> {
        let bytes = self.writing.borrow_mut().remove(part).unwrap_or_default();
        let mut writer = self.vault.part_writer(self.key).unwrap();
        writer.write(&bytes).unwrap();
        self.saved
            .borrow_mut()
            .insert(*part, writer.finish().unwrap().part());
        Ok(())
    }
}

impl SavedParts for Saving<'_> {
    fn saved(&self, part: &PartName) -> Result<Option<PartRef>, Error> {
        let saved = self.saved.borrow();
        let part = saved.get(part).ok_or(Error::Write(format!("{part:?}")))?;
        Ok(*part)
    }
}

/// Reads saved parts from a vault, as the host's `read-part` does.
struct VaultRead<'a>(&'a Vault<MemoryStore>);

impl AsyncRead for VaultRead<'_> {
    fn read(
        &self,
        part: PartRef,
        range: std::ops::Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + '_>> {
        let read = self
            .0
            .read_part(part, range.start, range.end - range.start)
            .map_err(|error| error.to_string());
        Box::pin(async move { read })
    }
}

#[test]
fn a_compaction_saves_its_merged_parts_through_a_part_writer() {
    let words = ["apples", "bananas", "cherries", "dates"];
    let mut content = SearchIndexContent::default();
    for (n, word) in words.iter().enumerate() {
        let n = n as u8 + 1;
        content = applied(&content, n, vec![put(n, i64::from(n), word)]);
    }
    assert_eq!(content.index().segment_count(), 4);

    let keys = Keyring::new();
    let key = uuid::Uuid::new_v4();
    keys.insert(key, ContentKey::random(), uuid::Uuid::new_v4());
    let vault = Vault::new(MemoryStore::new(), keys);
    let read = VaultRead(&vault);
    let (origin, now) = ([9; 16], 1_000);

    // What the library writes compacting the same index into memory.
    let memory = MemoryParts::default();
    let mut expected = content.index().clone();
    run(expected.compact(
        &content.reader([&content], &read),
        &memory,
        origin,
        now,
        MERGES_PER_PASS,
        u64::MAX,
        |_| 0,
    ))
    .unwrap();
    expected.forget_before(now - REMEMBERED);

    let mut compacted = content.clone();
    let out = Saving {
        vault: &vault,
        key,
        writing: RefCell::default(),
        saved: RefCell::default(),
    };
    run(compacted.compact(&read, &out, origin, now, u64::MAX)).unwrap();
    assert_eq!(compacted.index(), &expected);
    assert_eq!(compacted.index().segment_count(), 1);
    let mut saved = 0;
    for name in compacted.index().parts() {
        if matches!(name.kind, PartKind::Table(_) | PartKind::Deletions(_)) {
            continue;
        }
        let bytes = memory.parts.borrow()[&name].clone();
        match compacted.saved(&name) {
            Some(part) => {
                saved += 1;
                assert_eq!(vault.read_part(part, 0, part.length()).unwrap(), bytes);
            }
            None => assert!(bytes.is_empty(), "{name:?} was not saved"),
        }
    }
    assert!(saved >= 3, "only {saved} parts were saved");

    let search = |index: &SearchIndexContent, word: &str| -> Vec<u8> {
        let hits = run(index.search(&read, &Query::parse(word), 10, i64::MAX)).unwrap();
        hits.iter().map(|hit| hit.doc.block[0]).collect()
    };
    // Saved as the block's content and loaded again, it reads its merged parts from the vault.
    let manifest = vault
        .write(key, be_block_graph::SEARCH_INDEX_TYPE, compacted.parts())
        .unwrap();
    let loaded = SearchIndexContent::from_stored(&vault.load_first(&manifest).unwrap()).unwrap();
    for (n, word) in words.iter().enumerate() {
        assert_eq!(search(&content, word), [n as u8 + 1]);
        assert_eq!(search(&compacted, word), [n as u8 + 1]);
        assert_eq!(search(&loaded, word), [n as u8 + 1]);
    }
}
