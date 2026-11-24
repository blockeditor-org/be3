use std::sync::Mutex;

use super::*;

/// The owner takes two new phrases. One device of the guest read the account's block before
/// either and is slow to write what follows the first change; the other reads both, follows them
/// and writes first. The slow write lands last and is refused, since the block it writes over is
/// no longer the one it read, and the slow device is asked again of the block as it is now, so
/// the guest never goes back to the owner's middle key, which nothing leads on from.
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
    // Its write is refused on purpose: on a manual clock its backoff waits for no real time.
    harness.on_manual_clock(&slow).await;
    let block =
        crate::AccountRecords::open(&slow.connection(), slow.account(), slow.keys().identity())
            .await
            .unwrap()
            .expect("a person's account has its own block");

    // The slow device reads the block, and before it writes, the owner takes both phrases and
    // the fast device follows them. Its write, of the record following the first change alone,
    // is refused, and it is asked again of the block as the fast device left it.
    let reads: Mutex<Vec<Option<Vec<u8>>>> = Mutex::new(Vec::new());
    let phrases: Mutex<Option<be_block_keys::RecoverySecret>> = Mutex::new(None);
    block
        .update(|content| {
            let (reads, phrases, owner, fast, pinned) = (&reads, &phrases, &owner, &fast, &pinned);
            Box::pin(async move {
                let held = content.root().contact(owner.account()).map(<[u8]>::to_vec);
                let first_read = {
                    let mut reads = reads.lock().unwrap();
                    reads.push(held);
                    reads.len() == 1
                };
                if !first_read {
                    return Ok(Vec::new());
                }
                let first = new_phrase(
                    &owner.connection(),
                    owner.account(),
                    owner.keys().identity(),
                )
                .await;
                let second = new_phrase(&owner.connection(), owner.account(), &first).await;
                fast.refresh_contacts().await.unwrap();
                *phrases.lock().unwrap() = Some(second);
                // What the slow device would write having followed the first change alone.
                let mut card = pinned.card.clone();
                card.public = first.public();
                card.signing = first.signing();
                let stale = crate::contacts::OwnContact::new(card, pinned.verified);
                Ok(vec![be_block::AccountBlock::set_contact(
                    owner.account(),
                    Some(stale.kept()),
                )])
            })
        })
        .await
        .unwrap();
    let second = phrases.into_inner().unwrap().expect("the owner took both phrases");
    assert_eq!(
        fast.verified_identity(owner.account()),
        Some(second.identity())
    );
    let reads = reads.into_inner().unwrap();
    assert_eq!(reads.len(), 2, "the refused write was not asked again");
    let kept = reads[1]
        .as_deref()
        .and_then(|bytes| crate::contacts::KeptContact::decode(bytes, owner.account()))
        .and_then(crate::contacts::KeptContact::record)
        .expect("the fast device kept the owner's record");
    assert_eq!(
        kept.card.public,
        second.public(),
        "asked again, the slow device read the newest record"
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
