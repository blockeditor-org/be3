use super::*;

use std::time::Duration;

use be_block_graph::Access;

use crate::{Event, KeptEdit, OpenEdits};

/// Kept edits that do not read are set aside: the block's kept edits are empty for a replica
/// to open under a fresh origin, the set-aside ones stay as they were, and once their key
/// arrives they read and are told as recovered once, never given to a replica to send.
#[tokio::test]
async fn kept_edits_set_aside_are_recovered_once_their_key_arrives_and_never_sent() {
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
    guest.keep_sealed_open_edits(block, sealed.clone());

    guest.set_aside_open_edits(block).unwrap();
    assert!(guest.open_edits(block).is_none(), "a replica opens afresh");
    assert!(guest.recover_set_aside().is_empty(), "they do not read yet");

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
    assert_eq!(guest.recover_set_aside(), [(block, 2)]);
    assert!(guest.recover_set_aside().is_empty(), "told once");
    assert!(guest.open_edits(block).is_none(), "never given again");
}
