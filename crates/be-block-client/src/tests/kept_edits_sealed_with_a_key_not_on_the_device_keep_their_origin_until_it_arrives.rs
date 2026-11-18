use super::*;

use std::time::Duration;

use be_block_graph::Access;

use crate::{Event, KeptEdit, KeptSubmission, OpenEdits};

/// Edits a device kept of a block, sealed with a key it does not hold (its vault locked, as at
/// startup before the key arrives): the origin and counts are read all the same, the edits and
/// the submissions holding them fail as locked and stay kept as they are, and open once the
/// key arrives.
#[tokio::test]
async fn kept_edits_sealed_with_a_key_not_on_the_device_keep_their_origin_until_it_arrives() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let block = workspace(&owner, "Roadmap").await;
    owner
        .grant(block, guest.account(), Access::Edit)
        .await
        .unwrap();
    let listed = vec![guest.summary(block).await.unwrap()];
    guest.learn(&listed).await.unwrap();

    let edit = KeptEdit::Frame {
        editor: 1,
        bytes: b"typed".to_vec(),
    };
    let kept = OpenEdits {
        origin: 7,
        given_up: 2,
        submitted_through: 3,
        restarts: 1,
        ..OpenEdits::default()
    };
    let submission = KeptSubmission {
        counter: 9,
        through: 3,
        payload: b"submitted".to_vec(),
    };
    owner
        .keep_open_edits(
            block,
            kept.clone(),
            vec![edit.clone()],
            vec![submission.clone()],
        )
        .await
        .unwrap();
    let sealed = owner.sealed_open_edits(block).expect("kept");
    guest.keep_sealed_open_edits(block, sealed.clone());

    let (read, edits) = guest.open_edits(block).expect("the row is read");
    assert_eq!(
        (
            read.origin,
            read.given_up,
            read.submitted_through,
            read.restarts
        ),
        (7, 2, 3, 1)
    );
    assert!(matches!(edits, Err(ClientError::Locked(_))), "{edits:?}");
    assert_eq!(
        guest.sealed_open_edits(block),
        Some(sealed),
        "the edits stay kept as they were"
    );

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
    let (read, edits) = guest.open_edits(block).expect("the row is read");
    assert_eq!(read.origin, 7);
    assert_eq!(edits.unwrap(), (vec![edit], vec![submission]));
}
