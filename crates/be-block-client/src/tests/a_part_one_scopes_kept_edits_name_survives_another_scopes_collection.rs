use be_block_store::ObjectStore;
use be_type_text::TextContent;

use crate::{KeptEdit, OpenEdits};

use super::*;

/// Two accounts on one device share its store. A part one account's module wrote, named only
/// by an edit that account keeps, is that account's to collect: the other account's collection,
/// which knows nothing of the edit, leaves it alone, as it does every object another account's
/// part writers noted.
#[tokio::test]
async fn a_part_one_scopes_kept_edits_name_survives_another_scopes_collection() {
    let harness = Harness::start().await;
    let writer = harness.owner("writer@example.com").await;
    let other = harness.owner("other@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&writer, &refs, &store).await;
    let elsewhere = harness.device(&other, &refs, &store).await;
    let folder = workspace(&laptop, "Parts").await;
    let block = laptop
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    let mut part_writer = laptop.part_writer(block).await.unwrap();
    part_writer.write(&pseudorandom(16 * 1024, 31)).unwrap();
    let written = part_writer.finish().unwrap();
    let part = written.part().expect("bytes were written");
    let objects = laptop.part_objects(&[part]).unwrap();
    laptop
        .keep_open_edits(
            block,
            OpenEdits::default(),
            vec![KeptEdit::Frame {
                editor: 1,
                bytes: b"an edit naming the part".to_vec(),
            }],
            Vec::new(),
        )
        .await
        .unwrap();
    drop(written);

    assert_eq!(
        elsewhere.collect_unnamed(&Default::default()).unwrap(),
        0,
        "not the other account's to collect"
    );
    assert_eq!(
        laptop.collect_unnamed(&Default::default()).unwrap(),
        0,
        "the kept edit may name them"
    );
    for object in &objects {
        assert!(store.has(*object).unwrap(), "{object} stays");
    }

    harness.stop().await;
}
