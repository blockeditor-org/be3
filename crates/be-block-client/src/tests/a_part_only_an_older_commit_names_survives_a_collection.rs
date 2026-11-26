use be_block_content::Part;
use be_block_store::ObjectStore;
use be_type_text::TextContent;

use super::*;

/// A part a commit named and the next commit does not is still named by the older one, which
/// the device holds: a collection keeps it, and it reads back whole.
#[tokio::test]
async fn a_part_only_an_older_commit_names_survives_a_collection() {
    let harness = Harness::start().await;
    let owner = harness.owner("writer@example.com").await;
    let folder = workspace(&owner, "Parts").await;
    let block = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    let bytes = pseudorandom(16 * 1024, 12);
    let mut writer = owner.part_writer(block).await.unwrap();
    writer.write(&bytes).unwrap();
    // Pinned through the first commit, so its lookups leave the objects noted.
    let written = writer.finish().unwrap();
    let part = written.part().expect("bytes were written");
    let first = owner
        .write(
            block,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                Part::node(vec![Part::from(b"a header".as_slice()), Part::Reused(part)]),
                None,
            ),
        )
        .await
        .unwrap();
    let first = first.published().expect("the first commit published");
    let second = owner
        .write(
            block,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                Part::from(b"the part is gone from here".as_slice()),
                Some(first),
            ),
        )
        .await
        .unwrap();
    assert!(second.published().is_some(), "{second:?}");
    let objects = owner.part_objects(&[part]).unwrap();
    drop(written);
    let vault = owner.commits().vault();
    assert_eq!(
        vault.unnamed_count(),
        objects.len(),
        "noted until a collection"
    );

    assert_eq!(owner.collect_unnamed(&Default::default()).unwrap(), 0, "the older commit names it");
    assert_eq!(vault.unnamed_count(), 0, "found named, no longer noted");
    for object in &objects {
        assert!(vault.store().has(*object).unwrap(), "{object} stays");
    }
    assert_eq!(vault.read_part(part, 0, part.length()).unwrap(), bytes);

    harness.stop().await;
}
