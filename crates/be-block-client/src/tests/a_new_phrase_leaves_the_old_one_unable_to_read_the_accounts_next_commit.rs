use super::*;

/// An account takes a new phrase. Its block's earlier keys are sealed again to the new phrase,
/// so a device with it reads everything; but the block starts a new key epoch sealed to the new
/// phrase alone, so a device holding every key the old phrase opened reads nothing written after.
#[tokio::test]
async fn a_new_phrase_leaves_the_old_one_unable_to_read_the_accounts_next_commit() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.owner("guest@example.com").await;
    befriend(&rosa, &guest).await;

    let old = rosa.keys().identity().clone();
    // Another device of Rosa's, which keeps the old phrase.
    let kept = harness.second(&rosa).await;
    let held = crate::AccountRecords::open(&kept.connection(), rosa.account(), &old)
        .await
        .unwrap()
        .expect("a person's account has its own block");
    assert!(
        held.read()
            .await
            .unwrap()
            .root()
            .contact(guest.account())
            .is_some(),
        "the old phrase read the block before the change"
    );

    let fresh = new_phrase(&rosa.connection(), rosa.account(), &old).await;
    let renewed = crate::AccountRecords::open(&rosa.connection(), rosa.account(), &fresh)
        .await
        .unwrap()
        .expect("a person's account has its own block");
    assert!(
        renewed
            .read()
            .await
            .unwrap()
            .root()
            .contact(guest.account())
            .is_some(),
        "the new phrase reads what the old one wrote"
    );
    renewed
        .update(|_| {
            crate::peer::account_block::answered(vec![be_block::AccountBlock::set_contact(
                Uuid::new_v4(),
                Some(vec![1]),
            )])
        })
        .await
        .unwrap();

    let read = held.read().await;
    assert!(
        matches!(read, Err(ClientError::Locked(_))),
        "the old phrase read the block's next commit"
    );

    harness.stop().await;
}
