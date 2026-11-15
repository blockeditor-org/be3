use super::*;

use std::time::Duration;

use be_block_graph::Access;

use crate::{Event, KeptEdit, OpenEdits};

/// Kept edits that do not read are set aside: the block's kept edits are empty for a replica
/// to open under a fresh origin, a block keeps its newest set aside, and once their key
/// arrives they read and are told so once, left a tombstone, never given to a replica to send.
/// Forgetting the block drops them.
#[tokio::test]
async fn kept_edits_set_aside_are_bounded_told_once_they_read_and_never_sent() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let block = workspace(&owner, "Roadmap").await;
    owner
        .grant(block, guest.account(), Access::Edit)
        .await
        .unwrap();
    let listed = guest.list(Listing::Block(block)).await.unwrap();
    guest.learn(&listed).await.unwrap();

    let kept = OpenEdits {
        origin: 7,
        ..OpenEdits::default()
    };
    let edits = vec![
        KeptEdit::Frame {
            editor: 1,
            bytes: b"typed".to_vec(),
        },
        KeptEdit::Frame {
            editor: 1,
            bytes: b"more".to_vec(),
        },
    ];
    owner.keep_open_edits(block, kept, edits).await.unwrap();
    let sealed = owner.sealed_open_edits(block).expect("kept");
    for _ in 0..=crate::SetAside::KEPT {
        guest.keep_sealed_open_edits(block, sealed.clone());
        guest.set_aside_open_edits(block).unwrap();
    }
    assert_eq!(guest.set_aside_of(block).len(), crate::SetAside::KEPT, "the newest are kept");
    assert!(guest.open_edits(block).is_none(), "a replica opens afresh");
    assert!(guest.read_set_aside().is_empty(), "they do not read yet");

    let mut events = guest.connection().subscribe();
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .seal_share(block, guest.account(), &public)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), async {
        while !matches!(events.recv().await.unwrap(), Event::KeysChanged) {}
    })
    .await
    .expect("the server never told the guest of the key sealed to it");
    guest.keys_changed();
    guest.refresh_keys(Some(vec![block])).await.unwrap();
    assert_eq!(
        guest.read_set_aside(),
        vec![(block, 2); crate::SetAside::KEPT]
    );
    assert!(guest.read_set_aside().is_empty(), "told once");
    assert!(guest.open_edits(block).is_none(), "never given again");
    assert!(
        guest
            .set_aside_of(block)
            .iter()
            .all(|aside| aside.count == 2 && aside.kept.is_none()),
        "only tombstones are left"
    );

    guest.forget_cached(block).unwrap();
    assert!(guest.set_aside_of(block).is_empty(), "forgetting the block drops them");
}
