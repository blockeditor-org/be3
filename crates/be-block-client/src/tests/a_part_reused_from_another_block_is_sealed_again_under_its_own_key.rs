use be_block_graph::Access;
use be_block_store::ObjectStore;
use be_sdk_type_testing::sections::{SECTIONS, Sections};

use super::*;

#[tokio::test]
async fn a_part_reused_from_another_block_is_sealed_again_under_its_own_key() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let reader = harness.member(&owner, "reader@example.com").await;
    let private = workspace(&owner, "Private").await;
    let team = workspace(&owner, "Team").await;
    let source = owner
        .create::<Sections>(BlockParent::Block(private))
        .await
        .unwrap();
    let head = owner
        .save(source, &Sections::of(&["private"; SECTIONS]), None)
        .await
        .unwrap()
        .published()
        .expect("the source publishes");
    let manifest = owner.commits().get(head).unwrap().manifest;
    // Another device of the owner writes the copies without having read the source: it
    // fetches the reused parts' tops to learn whose they are, and then those parts whole.
    let owner = harness.second(&owner).await;

    // A part of the block's own, reused by a device that never fetched it, stays as it is.
    let stored = owner.load_first_part(&manifest).await.unwrap();
    let mut edited = Sections::from_stored(&stored).unwrap();
    let kept = edited.saved(7).expect("section 7 is a part");
    edited.set(0, "edited");
    let edit = owner
        .save(source, &edited, Some(head))
        .await
        .unwrap()
        .published()
        .expect("the edit publishes");
    assert!(
        !owner.commits().vault().store().has(kept.hash()).unwrap(),
        "saving the source downloaded a part of its own that it reuses"
    );
    let stored = owner
        .load_first_part(&owner.commits().get(edit).unwrap().manifest)
        .await
        .unwrap();
    assert_eq!(
        Sections::from_stored(&stored).unwrap().saved(7),
        Some(kept),
        "a part of the block's own keeps its ref"
    );

    // A checkout filled from another's snapshot writes that block's manifest whole.
    let copy = owner
        .create::<Sections>(BlockParent::Block(team))
        .await
        .unwrap();
    owner
        .write(copy, Write::written(manifest.clone(), None))
        .await
        .unwrap()
        .published()
        .expect("the copy publishes");
    // A merge over parts may reuse its sides' parts beside new bytes.
    let stored = owner.load_first_part(&manifest).await.unwrap();
    let mut merged = Sections::from_stored(&stored).unwrap();
    merged.set(0, "merged");
    let partly = owner
        .create::<Sections>(BlockParent::Block(team))
        .await
        .unwrap();
    owner
        .write(
            partly,
            Write::encoded(Sections::CONTENT_TYPE, merged.parts(), None),
        )
        .await
        .unwrap()
        .published()
        .expect("the partial reuse publishes");

    for block in [copy, partly] {
        let key = owner.summary(block).await.unwrap().key.unwrap();
        let written = owner
            .commits()
            .get(owner.local_head(block).unwrap())
            .unwrap()
            .manifest;
        for hash in owner.commits().vault().objects(&written).unwrap() {
            let sealed = owner.commits().vault().store().get(hash).unwrap().unwrap();
            assert_eq!(
                be_block_store::sealed_key(&sealed).unwrap(),
                key,
                "every object of a block is sealed under one of its own keys"
            );
        }
    }

    owner.refresh_contacts().await.unwrap();
    let public = owner.keys().contact(reader.account()).unwrap();
    owner
        .share(team, reader.account(), Access::View, &public)
        .await
        .unwrap();
    let copied = reader.open::<Sections>(copy).await.unwrap().unwrap();
    assert_eq!(copied.section(4), Some(&b"private"[..]));
    let partial = reader.open::<Sections>(partly).await.unwrap().unwrap();
    assert_eq!(partial.section(0), Some(&b"merged"[..]));
    assert_eq!(partial.section(9), Some(&b"private"[..]));
    harness.stop().await;
}
