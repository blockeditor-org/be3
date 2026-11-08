use be_block::Part;
use be_block_graph::Access;
use be_block_store::{Manifest, Node, ObjectStore};
use be_type_text::TextContent;

use super::*;

/// What an operation adding a part by its hash does across devices (`host.put-part`): one
/// member puts the part, and another saves a commit reusing it from the part's ref alone,
/// uploading none of it and never reading its bytes.
#[tokio::test]
async fn a_part_put_for_an_operation_is_saved_by_another_member_without_uploading_it() {
    let harness = Harness::start().await;
    let owner = harness.owner("indexer@example.com").await;
    let member = harness.member(&owner, "searcher@example.com").await;
    let index = workspace(&owner, "Index").await;
    let block = owner
        .create::<TextContent>(BlockParent::Block(index))
        .await
        .unwrap();
    let public = owner.keys().contact(member.account()).unwrap();
    owner
        .share(index, member.account(), Access::Edit, &public)
        .await
        .unwrap();

    let segment = pseudorandom(64 * 1024, 11);
    let part = owner.put_part(block, &segment).await.unwrap();
    let whole = Manifest {
        content_type: Uuid::nil(),
        root: Some(part),
    };
    let objects = owner.commits().vault().objects(&whole).unwrap();
    let chunks: Vec<_> = objects
        .iter()
        .copied()
        .filter(|hash| owner.commits().vault().get_value::<Node>(*hash).is_err())
        .collect();
    assert!(chunks.len() > 1, "the segment is saved in several chunks");

    let saved = member
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
    assert!(saved.published().is_some(), "{saved:?}");
    let store = member.commits().vault().store();
    for chunk in &chunks {
        assert!(
            !store.has(*chunk).unwrap(),
            "the member saved the part without its bytes, so it uploaded none of them"
        );
    }

    let reader = harness.second(&owner).await;
    let head = reader.manifest(block).await.unwrap().expect("a head");
    let mut expected = b"a header".to_vec();
    expected.extend_from_slice(&segment);
    assert_eq!(reader.read_manifest(&head).await.unwrap(), expected);
    assert_eq!(
        member
            .read_part(part, 0, segment.len() as u64)
            .await
            .unwrap(),
        segment,
        "the block holds the part, so its members read it"
    );

    harness.stop().await;
}
