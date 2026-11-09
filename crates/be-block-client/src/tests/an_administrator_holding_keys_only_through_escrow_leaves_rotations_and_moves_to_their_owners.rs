use be_sdk_type_testing::sections::{SECTIONS, Sections};

use super::*;

/// Records a rotation of `block` as the server does when someone stops being able to view it.
fn rotate_later(harness: &Harness, peer: &Peer<MemoryStore>, block: Uuid) {
    harness
        .database()
        .execute(
            "INSERT OR IGNORE INTO rotations (organization_id, block_id) VALUES (?1, ?2)",
            rusqlite::params![peer.organization().to_string(), block.to_string()],
        )
        .unwrap();
}

#[tokio::test]
async fn an_administrator_holding_keys_only_through_escrow_leaves_rotations_and_moves_to_their_owners()
 {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let private = workspace(&member, "Private").await;
    let block = member
        .create::<Sections>(BlockParent::Block(private))
        .await
        .unwrap();
    member
        .save(block, &Sections::of(&["kept"; SECTIONS]), None)
        .await
        .unwrap()
        .published()
        .expect("the sections publish");
    rotate_later(&harness, &member, private);

    let laptop = harness.second(&admin).await;
    assert_eq!(laptop.key_work().await.unwrap().rotations, [private]);
    let refused = laptop.tend().await.unwrap_err();
    assert!(
        matches!(refused, ClientError::Untrusted(_)),
        "an administrator's device that opens the workspace only through escrow rotated it: {refused}"
    );
    assert_eq!(
        laptop.key_work().await.unwrap().rotations,
        [private],
        "the rotation waits for a device that trusts every key"
    );
    let elsewhere = workspace(&laptop, "Elsewhere").await;
    let moved = laptop
        .set_parent(block, BlockParent::Block(elsewhere))
        .await
        .unwrap_err();
    assert!(
        matches!(moved, ClientError::Untrusted(_)),
        "an administrator's device moved a block whose keys it holds only through escrow: {moved}"
    );

    member.tend().await.unwrap();
    assert!(
        laptop.key_work().await.unwrap().rotations.is_empty(),
        "the owner's device did the rotation"
    );
    let read = harness
        .second(&admin)
        .await
        .open::<Sections>(block)
        .await
        .unwrap()
        .expect("the block has content");
    assert_eq!(read.section(3), Some(&b"kept"[..]));
    harness.stop().await;
}
