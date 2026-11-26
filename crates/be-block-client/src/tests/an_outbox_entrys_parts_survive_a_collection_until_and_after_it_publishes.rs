use be_block_content::Part;
use be_block_store::ObjectStore;
use be_type_text::TextContent;

use super::*;

/// A part written on a device that then stops stays noted across the restart (the refs keep
/// it), and a commit kept in the outbox offline names it: a collection leaves it, and once the
/// commit publishes, its head does.
#[tokio::test]
async fn an_outbox_entrys_parts_survive_a_collection_until_and_after_it_publishes() {
    let harness = Harness::start().await;
    let owner = harness.owner("writer@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let folder = workspace(&laptop, "Parts").await;
    let block = laptop
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    let head = laptop.newest_head(block).await.unwrap();
    let bytes = pseudorandom(16 * 1024, 9);
    let mut writer = laptop.part_writer(block).await.unwrap();
    writer.write(&bytes).unwrap();
    let part = writer.finish().unwrap().part().expect("bytes were written");
    let objects = laptop.part_objects(&[part]).unwrap();
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    assert_eq!(
        offline.commits().vault().unnamed_count(),
        objects.len(),
        "what the writer sealed is still noted after the restart"
    );
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

    assert_eq!(offline.collect_unnamed().unwrap(), 0);
    for object in &objects {
        assert!(store.has(*object).unwrap(), "the outbox entry's {object} stays");
    }

    harness.reconnect(&offline, owner.token()).await;
    offline.drain(|_| false).await.unwrap();
    assert_eq!(offline.pending().commits, 0, "the commit published");
    assert_eq!(offline.collect_unnamed().unwrap(), 0);
    assert_eq!(
        offline
            .commits()
            .vault()
            .read_part(part, 0, part.length())
            .unwrap(),
        bytes
    );

    harness.stop().await;
}
