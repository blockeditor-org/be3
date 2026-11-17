use super::*;

/// An administrator's device that stops after opening an invitation's accepted link, before it
/// signs the accepter in, still has the link to finish: the next `finish_links` signs them into
/// the organization's log and deletes it.
#[tokio::test]
async fn an_invitation_finished_halfway_signs_its_accepter_in_on_the_next_finish() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let own = harness.owner("member@example.com").await;
    let card = crate::contacts::card(
        admin.account(),
        admin.keys().identity(),
        "Admin",
        "admin@example.com",
    );
    let code = crate::contacts::invite(
        &admin.connection(),
        admin.keys().identity(),
        &card,
        admin.organization(),
        &crate::contacts::Pinned {
            root: admin.keys().organization_root(),
            escrow: admin.keys().escrow(),
        },
    )
    .await
    .unwrap();
    let theirs = crate::contacts::card(
        own.account(),
        own.keys().identity(),
        "Member",
        "member@example.com",
    );
    crate::contacts::accept_link(
        &own.connection(),
        own.keys().identity(),
        &theirs,
        &code,
        crate::contacts::Expected::Invitation,
    )
    .await
    .unwrap();
    let member = own.account();
    let signed_in = || async {
        admin
            .checked_organization()
            .await
            .unwrap()
            .expect("the organization has a log")
            .member(member.into_bytes())
            .is_some()
    };
    let listed = || async {
        be_block_protocol::read::<Vec<be_block_protocol::OwnLink>, _>(
            &admin.connection(),
            None,
            be_block_protocol::Query::Links,
        )
        .await
        .unwrap()
        .unwrap()
        .iter()
        .any(|link| link.accepter == Some(member))
    };

    // The device opens the link and stops before signing the member in.
    let opened = crate::contacts::finish_links(
        &admin.connection(),
        admin.account(),
        admin.keys().identity(),
        admin.organization(),
    )
    .await
    .unwrap();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(!signed_in().await, "the member was signed in halfway");
    assert!(
        listed().await,
        "the link was deleted before it was finished"
    );

    admin.finish_links().await.unwrap();
    assert!(
        signed_in().await,
        "the next finish did not sign the member in"
    );
    assert!(!listed().await, "the finished link is still listed");

    harness.stop().await;
}
