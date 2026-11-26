use super::*;

/// What `peer`'s device lists of `account`, as it pinned it.
async fn record(peer: &Peer<MemoryStore>, account: Uuid) -> crate::contacts::OwnContact {
    peer.refresh_contacts()
        .await
        .unwrap()
        .into_iter()
        .find(|contact| contact.account == account)
        .and_then(|contact| contact.record())
        .unwrap()
}

/// The server serves the accepting device an account block holding an older record of a contact
/// than the one this device pinned, and the contact sends a new link: the record the link makes
/// follows this device's pin as well as the block, so the device does not read its own new link
/// as a rollback (guides/blocks/keys.md, "Contacts").
#[tokio::test]
async fn a_link_accepted_after_the_server_serves_an_older_contact_record_outranks_this_devices_pin()
{
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.member(&rosa, "guest@example.com").await;
    let old = record(&guest, rosa.account()).await;
    let block = crate::AccountRecords::open(
        &guest.connection(),
        guest.account(),
        guest.keys().identity(),
    )
    .await
    .unwrap()
    .expect("a person's account has its own block");
    // A newer record this device pins, then a block that holds the older again, as a server that
    // serves an older version of the block would.
    let newer = crate::contacts::OwnContact {
        verified: old.verified + 10_000_000,
        ..old.clone()
    };
    block
        .update(|_| {
            crate::peer::account_block::answered(vec![be_block::AccountBlock::set_contact(
                rosa.account(),
                Some(newer.kept()),
            )])
        })
        .await
        .unwrap();
    assert_eq!(
        record(&guest, rosa.account()).await.verified,
        newer.verified
    );
    block
        .update(|_| {
            crate::peer::account_block::answered(vec![be_block::AccountBlock::set_contact(
                rosa.account(),
                Some(old.kept()),
            )])
        })
        .await
        .unwrap();

    let code = crate::contacts::make_link(
        &rosa.connection(),
        rosa.keys().identity(),
        &crate::contacts::card(
            rosa.account(),
            rosa.keys().identity(),
            "Rosa",
            "rosa@example.com",
        ),
    )
    .await
    .unwrap();
    crate::contacts::accept_link(
        &guest.connection(),
        guest.keys().identity(),
        &crate::contacts::card(
            guest.account(),
            guest.keys().identity(),
            "guest@example.com",
            "guest@example.com",
        ),
        &code,
        crate::contacts::Expected::Contact,
        &guest.contact_pins(),
    )
    .await
    .unwrap();

    let listed = guest.refresh_contacts().await.unwrap();
    let contact = listed
        .iter()
        .find(|contact| contact.account == rosa.account())
        .unwrap();
    assert_eq!(
        contact.mismatch, None,
        "this device read its own new link as a rollback"
    );
    assert!(contact.verified > newer.verified);

    harness.stop().await;
}
