use super::*;

use be_block_content::{SEARCH_INDEX, be_block_model::BlockRef};
use be_block_graph::{Access, SEARCH_INDEX_TYPE};

async fn index_bytes(peer: &Peer<MemoryStore>, index: Uuid) -> Vec<u8> {
    let head = peer
        .newest_head(index)
        .await
        .unwrap()
        .expect("the index has content");
    let commit = peer.load_commit(head).await.unwrap();
    peer.read_manifest(&commit.manifest).await.unwrap()
}

#[tokio::test]
async fn removing_a_member_rotates_a_workspaces_search_index_with_it() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let removed = harness.member(&admin, "removed@example.com").await;
    let team = workspace(&admin, "Team").await;
    let index = admin
        .create_block(
            Uuid::now_v7(),
            SEARCH_INDEX_TYPE,
            BlockParent::Block(team),
            &be_block_content::BlockMetadata::default(),
            false,
        )
        .await
        .unwrap()
        .id;
    admin
        .define(team, |metadata| {
            metadata.insert(&SEARCH_INDEX, &BlockRef(index))
        })
        .await
        .unwrap();
    let before = admin
        .write(
            index,
            crate::Write::encoded(SEARCH_INDEX_TYPE, b"before".as_slice(), None),
        )
        .await
        .unwrap()
        .published();
    for member in [&reader, &removed] {
        let public = admin.keys().contact(member.account()).unwrap();
        admin
            .share(team, member.account(), Access::View, &public)
            .await
            .unwrap();
    }
    assert_eq!(index_bytes(&removed, index).await, b"before");
    let key = admin.current_key(index);
    assert!(key.is_some());

    admin.remove_member(removed.account()).await.unwrap();
    assert!(admin.key_work().await.unwrap().rotations.contains(&team));
    admin.tend().await.unwrap();
    assert!(
        admin.key_work().await.unwrap().rotations.is_empty(),
        "a rotation was left undone"
    );
    assert_ne!(
        admin.current_key(index),
        key,
        "the index kept the key a removed member held"
    );
    admin
        .write(
            index,
            crate::Write::encoded(SEARCH_INDEX_TYPE, b"after".as_slice(), before),
        )
        .await
        .unwrap();
    assert_eq!(index_bytes(&reader, index).await, b"after");

    harness.stop().await;
}
