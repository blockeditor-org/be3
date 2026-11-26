use std::collections::HashSet;

use be_block_store::ObjectStore;
use be_type_text::TextContent;

use super::*;

/// What a part writer still open has sealed, and the part it finished while that is held, are
/// pinned: a collection leaves them. Once let go of, with nothing naming them, the next one
/// removes them all.
#[tokio::test]
async fn an_open_part_writers_chunks_survive_a_collection() {
    let harness = Harness::start().await;
    let owner = harness.owner("writer@example.com").await;
    let folder = workspace(&owner, "Parts").await;
    let block = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    let store = owner.commits().vault().store().clone();
    let held: HashSet<_> = store.hashes().into_iter().collect();
    let sealed = |store: &MemoryStore| -> Vec<_> {
        store
            .hashes()
            .into_iter()
            .filter(|object| !held.contains(object))
            .collect()
    };

    let bytes = pseudorandom(16 * 1024, 5);
    let (first, rest) = bytes.split_at(8 * 1024);
    let mut writer = owner.part_writer(block).await.unwrap();
    writer.write(first).unwrap();
    let so_far = sealed(&store);
    assert!(!so_far.is_empty(), "the writer sealed chunks as it went");
    assert_eq!(owner.collect_unnamed().unwrap(), 0);
    for object in &so_far {
        assert!(
            store.has(*object).unwrap(),
            "the open writer's {object} stays"
        );
    }

    writer.write(rest).unwrap();
    let written = writer.finish().unwrap();
    let part = written.part().expect("bytes were written");
    assert_eq!(owner.collect_unnamed().unwrap(), 0);
    assert_eq!(
        owner
            .commits()
            .vault()
            .read_part(part, 0, part.length())
            .unwrap(),
        bytes,
        "the finished part, still held, reads back whole"
    );

    drop(written);
    let objects = sealed(&store);
    assert_eq!(owner.collect_unnamed().unwrap(), objects.len());
    for object in &objects {
        assert!(
            !store.has(*object).unwrap(),
            "{object}, named by nothing, is removed"
        );
    }
    assert_eq!(owner.commits().vault().unnamed_count(), 0);

    harness.stop().await;
}
