use be_block_graph::Access;
use be_block_store::ObjectStore;
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

async fn sections_in(peer: &Peer<MemoryStore>, parent: Uuid, text: &str) -> Uuid {
    let block = peer
        .create::<Sections>(BlockParent::Block(parent))
        .await
        .unwrap();
    peer.save(block, &Sections::of(&[text; SECTIONS]), None)
        .await
        .unwrap()
        .published()
        .expect("the sections publish");
    block
}

/// Whether every object of `block`'s head is sealed under a key `peer` trusts.
async fn sealed_by(peer: &Peer<MemoryStore>, block: Uuid) -> bool {
    let manifest = peer.manifest(block).await.unwrap().unwrap();
    let vault = peer.commits().vault();
    for hash in peer.objects_of(&manifest).await.unwrap() {
        peer.fetch(&[hash]).await.unwrap();
        let sealed = vault.store().get(hash).unwrap().unwrap();
        if !peer
            .keys()
            .trusted(be_block_store::sealed_key(&sealed).unwrap())
        {
            return false;
        }
    }
    true
}

async fn share(owner: &Peer<MemoryStore>, block: Uuid, member: &Peer<MemoryStore>) {
    owner.refresh_contacts().await.unwrap();
    let public = owner.keys().contact(member.account()).unwrap();
    owner
        .share(block, member.account(), Access::View, &public)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_administrator_rotates_and_moves_what_it_holds_only_through_escrow() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let private = workspace(&member, "Private").await;
    let rotated = sections_in(&member, private, "rotated").await;
    let other = workspace(&member, "Other").await;
    let moved = sections_in(&member, other, "moved").await;
    rotate_later(&harness, &member, private);
    // The member leaves: their only device is gone, so no device trusts their keys.
    drop(member);

    let laptop = harness.second(&admin).await;
    assert!(
        !sealed_by(&laptop, rotated).await,
        "the admin holds the member's keys through escrow alone"
    );
    laptop.tend().await.unwrap();
    assert!(
        laptop.key_work().await.unwrap().rotations.is_empty(),
        "the administrator's device did the rotation nobody else could"
    );
    assert!(
        sealed_by(&laptop, rotated).await,
        "the rotation took the head off the epochs the administrator could not rewrap"
    );
    let viewer = harness.member(&admin, "viewer@example.com").await;
    share(&laptop, private, &viewer).await;
    let read = viewer.open::<Sections>(rotated).await.unwrap().unwrap();
    assert_eq!(read.section(6), Some(&b"rotated"[..]));

    let elsewhere = workspace(&laptop, "Elsewhere").await;
    laptop
        .set_parent(moved, BlockParent::Block(elsewhere))
        .await
        .unwrap();
    assert!(sealed_by(&laptop, moved).await);
    let reader = harness.member(&admin, "reader@example.com").await;
    share(&laptop, elsewhere, &reader).await;
    let read = reader.open::<Sections>(moved).await.unwrap().unwrap();
    assert_eq!(read.section(2), Some(&b"moved"[..]));
    harness.stop().await;
}
