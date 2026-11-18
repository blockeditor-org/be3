use super::*;

/// The owner takes two new phrases. One device of the guest read the account's block before
/// either and is slow to write what follows the first change; the other reads both, follows them
/// and writes first. The slow write lands last and is refused, since the block it writes over is
/// no longer the one it read, so the guest never goes back to the owner's middle key, which
/// nothing leads on from.
#[tokio::test]
async fn a_device_that_read_fewer_key_changes_never_rolls_a_contact_back() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let fast = harness.member(&owner, "guest@example.com").await;
    let slow = harness.second(&fast).await;
    let pinned = slow
        .refresh_contacts()
        .await
        .unwrap()
        .into_iter()
        .find(|contact| contact.account == owner.account())
        .and_then(|contact| contact.record())
        .unwrap();
    let block =
        crate::AccountRecords::open(&slow.connection(), slow.account(), slow.keys().identity())
            .await
            .unwrap()
            .expect("a person's account has its own block");
    let read = block.read().await.unwrap();

    let first = new_phrase(
        &owner.connection(),
        owner.account(),
        owner.keys().identity(),
    )
    .await;
    // What the slow device would write having followed the first change alone.
    let mut card = pinned.card.clone();
    card.public = first.public();
    card.signing = first.signing();
    let stale = crate::contacts::OwnContact::new(card, pinned.verified);

    let second = new_phrase(&owner.connection(), owner.account(), &first).await;
    fast.refresh_contacts().await.unwrap();
    assert_eq!(
        fast.verified_identity(owner.account()),
        Some(second.identity())
    );

    let landed = block
        .save(
            read,
            &[be_block::AccountBlock::set_contact(
                owner.account(),
                Some(stale.kept()),
            )],
        )
        .await;
    assert!(
        matches!(landed, Err(ClientError::Contended)),
        "the slow write went over the newer record: {landed:?}"
    );
    let listed = slow.refresh_contacts().await.unwrap();
    let held = listed
        .iter()
        .find(|contact| contact.account == owner.account())
        .unwrap();
    assert_eq!(held.public, Some(second.public()), "the record rolled back");
    assert_eq!(
        slow.verified_identity(owner.account()),
        Some(second.identity()),
        "the slow device, reading again, follows to the newest key"
    );

    harness.stop().await;
}
