use super::*;

/// A new account, whose block was never made, accepts two contacts' links, leaving their records
/// as letters in its mailbox. Both of its devices then take the mailbox at once: both make the
/// block and write it, the one refused reads it again, and the block ends up keeping both
/// records, with both letters gone from the mailbox.
#[tokio::test]
async fn two_devices_of_a_new_account_keep_both_its_letters_signing_in_at_once() {
    let harness = Harness::start().await;
    let alice = harness.owner("alice@example.com").await;
    let bob = harness.owner("bob@example.com").await;
    // In no organization of its own, whose pin would make its block.
    let rosa = super::a_share_link_grants_view_to_an_account_no_one_granted::personal(
        &harness,
        "rosa@example.com",
    )
    .await;
    let other = harness.second(&rosa).await;
    // Their writes contend: on a manual clock their backoff waits for no real time.
    harness.on_manual_clock(&rosa).await;
    harness.on_manual_clock(&other).await;

    let me = crate::contacts::card(
        rosa.account(),
        rosa.keys().identity(),
        "Rosa",
        "rosa@example.com",
    );
    for (maker, email) in [(&alice, "alice@example.com"), (&bob, "bob@example.com")] {
        let code = crate::contacts::make_link(
            &maker.connection(),
            maker.keys().identity(),
            &crate::contacts::card(maker.account(), maker.keys().identity(), email, email),
            None,
        )
        .await
        .unwrap();
        crate::contacts::accept_link(
            &rosa.connection(),
            rosa.keys().identity(),
            &me,
            &code,
            crate::contacts::Expected::Contact,
            &Default::default(),
        )
        .await
        .unwrap();
    }
    let letters = |mailbox: &[be_block_protocol::Message]| -> Vec<u64> {
        mailbox
            .iter()
            .filter(|message| matches!(message.letter, be_block_protocol::Letter::Contact(_)))
            .map(|message| message.id)
            .collect()
    };
    let waiting = letters(&crate::contacts::mailbox(&rosa.connection()).await.unwrap());
    assert_eq!(waiting.len(), 2, "each link left its record as a letter");
    let block =
        crate::AccountRecords::open(&rosa.connection(), rosa.account(), rosa.keys().identity())
            .await
            .unwrap()
            .expect("a person's account has its own block");
    assert!(
        block
            .read()
            .await
            .unwrap()
            .root()
            .contact(alice.account())
            .is_none(),
        "the block keeps nothing before the letters are taken"
    );

    let (one, two) = tokio::join!(rosa.refresh_contacts(), other.refresh_contacts());
    one.unwrap();
    two.unwrap();

    let kept = block.read().await.unwrap().root();
    for maker in [&alice, &bob] {
        assert!(
            kept.contact(maker.account()).is_some(),
            "the block lost a record a letter carried"
        );
    }
    assert_eq!(
        letters(&crate::contacts::mailbox(&rosa.connection()).await.unwrap()),
        Vec::<u64>::new(),
        "a letter kept in the block stayed in the mailbox"
    );
    for device in [&rosa, &other] {
        let listed = device.refresh_contacts().await.unwrap();
        for maker in [&alice, &bob] {
            assert!(
                listed
                    .iter()
                    .any(|contact| contact.account == maker.account())
            );
        }
    }

    harness.stop().await;
}
