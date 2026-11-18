use super::*;

/// A contact connects again by a new link, as one who lost their phrase must, and one device
/// accepts it; a device of the same account that pinned a record dated by a clock running
/// ahead takes the new record all the same, as a record a link makes follows the newest the
/// account's block holds
/// (guides/blocks/keys.md, "Contacts").
#[tokio::test]
async fn a_contact_connected_again_by_a_new_link_outranks_every_devices_pin() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let ahead = harness.member(&rosa, "guest@example.com").await;
    ahead.refresh_contacts().await.unwrap();
    // This device's clock runs far ahead: the record it pins is dated well past every other.
    let pinned = ahead
        .refresh_contacts()
        .await
        .unwrap()
        .into_iter()
        .find(|contact| contact.account == rosa.account())
        .and_then(|contact| contact.record())
        .unwrap();
    let dated_ahead = crate::contacts::OwnContact {
        verified: pinned.verified + 10_000_000,
        ..pinned
    };
    crate::AccountRecords::open(
        &ahead.connection(),
        ahead.account(),
        ahead.keys().identity(),
    )
    .await
    .unwrap()
    .expect("a person's account has its own block")
    .update(|_| {
        vec![be_block::AccountBlock::set_contact(
            rosa.account(),
            Some(dated_ahead.kept()),
        )]
    })
    .await
    .unwrap();
    ahead.refresh_contacts().await.unwrap();

    // Rosa sends a new link, as one who lost her phrase would, and the other device takes it.
    let code = crate::contacts::make_link(
        &rosa.connection(),
        rosa.keys().identity(),
        &crate::contacts::card(
            rosa.account(),
            rosa.keys().identity(),
            "Rosa",
            "rosa@example.com",
        ),
        None,
    )
    .await
    .unwrap();
    let behind = harness.second(&ahead).await;
    crate::contacts::accept_link(
        &behind.connection(),
        behind.keys().identity(),
        &crate::contacts::card(
            behind.account(),
            behind.keys().identity(),
            "guest@example.com",
            "guest@example.com",
        ),
        &code,
        crate::contacts::Expected::Contact,
    )
    .await
    .unwrap();

    let listed = ahead.refresh_contacts().await.unwrap();
    let contact = listed
        .iter()
        .find(|contact| contact.account == rosa.account())
        .unwrap();
    assert_eq!(
        contact.mismatch, None,
        "the new link's record read as a rollback"
    );
    assert!(contact.verified > dated_ahead.verified);
    assert_eq!(
        contact.display_name, "Rosa",
        "the new link's card is the one kept"
    );

    harness.stop().await;
}
