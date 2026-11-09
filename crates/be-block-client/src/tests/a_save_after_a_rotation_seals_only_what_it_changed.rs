use std::collections::HashSet;

use be_block_graph::Access;
use be_block_store::ObjectStore;
use be_sdk_type_testing::sections::{SECTIONS, Sections};

use super::*;

const TEXTS: [&str; SECTIONS] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

async fn read(peer: &Peer<MemoryStore>, block: Uuid) -> Vec<Vec<u8>> {
    let read = peer
        .open::<Sections>(block)
        .await
        .unwrap()
        .expect("the block has content");
    (0..SECTIONS)
        .map(|index| read.section(index).expect("a section").to_vec())
        .collect()
}

fn texts(changed: &[(usize, &str)]) -> Vec<Vec<u8>> {
    let mut texts: Vec<Vec<u8>> = TEXTS.iter().map(|text| text.as_bytes().to_vec()).collect();
    for (index, text) in changed {
        texts[*index] = text.as_bytes().to_vec();
    }
    texts
}

async fn share(owner: &Peer<MemoryStore>, block: Uuid, member: &Peer<MemoryStore>) {
    let public = owner.keys().contact(member.account()).unwrap();
    owner
        .share(block, member.account(), Access::View, &public)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_save_after_a_rotation_seals_only_what_it_changed() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let leaver = harness.member(&owner, "leaver@example.com").await;
    let team = workspace(&owner, "Team").await;
    let block = owner
        .create::<Sections>(BlockParent::Block(team))
        .await
        .unwrap();
    let first = owner
        .save(block, &Sections::of(&TEXTS), None)
        .await
        .unwrap()
        .published()
        .expect("the sections publish");
    let old_key = owner.summary(block).await.unwrap().key.unwrap();
    share(&owner, team, &leaver).await;
    owner
        .grant(team, leaver.account(), Access::None)
        .await
        .unwrap();
    owner.tend().await.unwrap();
    let new_key = owner.summary(block).await.unwrap().key.unwrap();
    assert_ne!(new_key, old_key, "the rotation gave the block a new key");

    let mut sections = owner.open::<Sections>(block).await.unwrap().unwrap();
    sections.set(4, "FOUR");
    let second = owner
        .save(block, &sections, Some(first))
        .await
        .unwrap()
        .published()
        .expect("the edit publishes");
    let vault = owner.commits().vault();
    let manifest = |commit| owner.commits().get(commit).unwrap().manifest;
    let old: HashSet<_> = vault.objects(&manifest(first)).unwrap().into_iter().collect();
    let new: HashSet<_> = vault
        .objects_besides(&manifest(second), &old)
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(
        new.len(),
        3,
        "the save after the rotation wrote the changed section, the header and the root, and resealed nothing"
    );
    for hash in vault.objects(&manifest(second)).unwrap() {
        let sealed = owner.commits().vault().store().get(hash).unwrap().unwrap();
        let key = be_block_store::sealed_key(&sealed).unwrap();
        match new.contains(&hash) {
            true => assert_eq!(key, new_key, "what the save wrote is under the new key"),
            false => assert_eq!(key, old_key, "a kept section stays under its old key"),
        }
    }

    let viewer = harness.member(&owner, "viewer@example.com").await;
    share(&owner, team, &viewer).await;
    assert_eq!(
        read(&viewer, block).await,
        texts(&[(4, "FOUR")]),
        "a viewer let in after the rotation reads the parts under the old key too"
    );

    let other = workspace(&owner, "Other").await;
    let elsewhere = harness.member(&owner, "elsewhere@example.com").await;
    share(&owner, other, &elsewhere).await;
    owner
        .set_parent(block, BlockParent::Block(other))
        .await
        .unwrap();
    assert_eq!(
        read(&elsewhere, block).await,
        texts(&[(4, "FOUR")]),
        "a member of the scope the block moved into reads every epoch its head is under"
    );

    let mut sections = owner.open::<Sections>(block).await.unwrap().unwrap();
    sections.set(7, "SEVEN");
    owner
        .save(block, &sections, owner.local_head(block))
        .await
        .unwrap()
        .published()
        .expect("the save after the move publishes");
    assert_eq!(
        read(&elsewhere, block).await,
        texts(&[(4, "FOUR"), (7, "SEVEN")]),
        "a save after the move stays readable in the new scope"
    );
    harness.stop().await;
}
