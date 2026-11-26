use std::collections::HashSet;

use be_block_content::Part;
use be_block_store::ObjectStore;
use be_type_text::TextContent;

use super::*;

/// Sealing is convergent, so a part let go of can share chunks with one a commit names: a
/// collection removes only the objects nothing names, and the named part reads back whole.
#[tokio::test]
async fn a_chunk_a_named_part_shares_survives_a_collection() {
    let harness = Harness::start().await;
    let owner = harness.owner("writer@example.com").await;
    let folder = workspace(&owner, "Parts").await;
    let block = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    let bytes = pseudorandom(16 * 1024, 7);

    let mut writer = owner.part_writer(block).await.unwrap();
    writer.write(&bytes).unwrap();
    let written = writer.finish().unwrap();
    let named = written.part().expect("bytes were written");
    let saved = owner
        .write(
            block,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                Part::node(vec![
                    Part::from(b"a header".as_slice()),
                    Part::Reused(named),
                ]),
                None,
            ),
        )
        .await
        .unwrap();
    assert!(saved.published().is_some(), "{saved:?}");
    drop(written);

    // The same bytes and more, let go of before anything names them.
    let mut writer = owner.part_writer(block).await.unwrap();
    writer.write(&bytes).unwrap();
    writer.write(&pseudorandom(8 * 1024, 8)).unwrap();
    let written = writer.finish().unwrap();
    let let_go = written.part().expect("bytes were written");
    let kept: HashSet<_> = owner.part_objects(&[named]).unwrap().into_iter().collect();
    let dropped: Vec<_> = owner.part_objects(&[let_go]).unwrap();
    drop(written);
    assert!(
        dropped.iter().any(|object| kept.contains(object)),
        "the two parts share a chunk"
    );

    let unnamed: Vec<_> = dropped
        .iter()
        .filter(|object| !kept.contains(object))
        .copied()
        .collect();
    assert_eq!(owner.collect_unnamed().unwrap(), unnamed.len());
    let store = owner.commits().vault().store();
    for object in &unnamed {
        assert!(
            !store.has(*object).unwrap(),
            "{object}, named by nothing, is removed"
        );
    }
    for object in &kept {
        assert!(
            store.has(*object).unwrap(),
            "the named part's {object} stays"
        );
    }
    assert_eq!(
        owner
            .commits()
            .vault()
            .read_part(named, 0, named.length())
            .unwrap(),
        bytes
    );

    harness.stop().await;
}
