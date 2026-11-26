use be_block_content::Part;
use be_block_protocol::{Journal, JournalBase, JournalEntry};
use be_block_store::{Hash, ObjectStore};
use be_type_text::TextContent;

use super::*;

/// A module's part named only by an entry of the block's journal past its base (no edit kept or
/// set aside, no commit naming it yet) survives collections across a restart, offline; the
/// entry then seals a commit that reuses the part, which reads back whole. Collected, the part
/// would be lost: the server never journaled the entry, so a fetch would not find it named.
#[tokio::test]
async fn a_part_only_journal_entries_name_survives_collections_until_they_seal() {
    let harness = Harness::start().await;
    let owner = harness.owner("writer@example.com").await;
    let file = RefsFile::new();
    let refs = file.open();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let folder = workspace(&laptop, "Parts").await;
    let block = laptop
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    let head = laptop.newest_head(block).await.unwrap();
    let bytes = pseudorandom(16 * 1024, 13);
    let mut writer = laptop.part_writer(block).await.unwrap();
    writer.write(&bytes).unwrap();
    let written = writer.finish().unwrap();
    let part = written.part().expect("bytes were written");
    let objects = laptop.part_objects(&[part]).unwrap();
    // The edit naming the part, numbered past the journal's base before the host stops.
    let journal = Journal {
        tag: Hash::of(b"the text's module"),
        base: JournalBase {
            commit: head,
            sequence: 0,
            state: Vec::new(),
            parts: Vec::new(),
        },
        entries: vec![JournalEntry {
            sequence: 1,
            origin: 1,
            counter: 1,
            base: 0,
            floor: 0,
            received: Some(1_000),
            payload: b"an edit naming the part".to_vec(),
        }],
        lost: false,
    };
    laptop.keep_journal(block, &journal).unwrap();
    drop(written);
    drop(laptop);
    // The host stops: the database is opened again, its unnamed parts' notes read from it.
    drop(refs);
    let refs = file.open();

    let offline = harness.offline(&owner, &refs, &store);
    let vault = offline.commits().vault();
    assert_eq!(
        vault.unnamed_count(),
        objects.len(),
        "still noted after the restart"
    );
    assert_eq!(
        offline.collect_unnamed(&Default::default()).unwrap(),
        0,
        "the journal's entry may name them"
    );
    for object in &objects {
        assert!(store.has(*object).unwrap(), "{object} stays");
    }

    let saved = offline
        .write(
            block,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                Part::node(vec![Part::from(b"a header".as_slice()), Part::Reused(part)]),
                head,
            ),
        )
        .await
        .unwrap();
    assert!(matches!(saved, Saved::Kept(_)), "{saved:?}");
    assert_eq!(offline.collect_unnamed(&Default::default()).unwrap(), 0);
    assert_eq!(vault.read_part(part, 0, part.length()).unwrap(), bytes);

    harness.stop().await;
}
