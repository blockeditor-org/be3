use super::*;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn edits_to_a_block_whose_access_was_lost_wait_aside_until_it_returns() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let member = harness.member(&owner, "member@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let base = owner
        .save(note, &TextBlock::of("shared"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let public = owner.keys().contact(member.account()).unwrap();
    owner
        .share(team, member.account(), Access::Edit, &public)
        .await
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&member, &refs, &store).await;
    assert_eq!(text(&laptop, note).await, "shared");
    drop(laptop);

    let offline = harness.offline(&member, &refs, &store);
    assert!(
        offline
            .save(note, &TextBlock::of("written while away"), Some(base))
            .await
            .is_err()
    );
    let unsent = offline.local_head(note).unwrap();

    owner
        .grant(team, member.account(), Access::None)
        .await
        .unwrap();
    owner.tend().await.unwrap();

    harness.reconnect(&offline, member.token()).await;
    assert_eq!(
        offline.drain(|_| false).await.unwrap(),
        [Drained::Unsent { block: note }]
    );
    let counts = offline.pending();
    assert_eq!((counts.operations, counts.unsent), (0, 1));
    let aside = offline.unsent();
    assert_eq!(aside.len(), 1);
    assert_eq!(aside[0].commits, [unsent]);
    assert!(!aside[0].offerable);
    assert!(
        offline.commits().try_get(unsent).unwrap().is_some(),
        "the edit made while away was deleted"
    );

    owner
        .share(team, member.account(), Access::Edit, &public)
        .await
        .unwrap();
    offline.summary(note).await.unwrap();
    assert!(offline.unsent()[0].offerable);
    assert!(offline.restore_unsent(note).unwrap());
    assert!(offline.unsent().is_empty());
    let drained = offline.drain(|_| false).await.unwrap();
    assert_eq!(
        drained,
        [Drained::Published {
            block: note,
            head: unsent
        }]
    );
    assert_eq!(text(&owner, note).await, "written while away");

    harness.stop().await;
}
