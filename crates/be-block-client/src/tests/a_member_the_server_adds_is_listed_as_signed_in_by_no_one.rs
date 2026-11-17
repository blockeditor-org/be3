use super::*;

/// A member joins the organization's log only by a record an administrator's device signs, from
/// the invitation it made (or an agent's maker). One the server adds on its own is in no record:
/// administrators' key work lists it, so does its own, and no administrator's device signs it
/// in, so the escrow wraps it writes wait.
#[tokio::test]
async fn a_member_the_server_adds_is_listed_as_signed_in_by_no_one() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let invited = harness.member(&admin, "invited@example.com").await;
    let organization = admin.organization();

    let own = harness.owner("added@example.com").await;
    harness
        .database()
        .execute(
            "INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, 'editor')",
            rusqlite::params![organization.to_string(), own.account().to_string()],
        )
        .unwrap();
    let added = Peer::connect(
        PeerConfig::new(
            &harness.url,
            own.keys().identity().clone(),
            Credentials::Token(own.token().to_owned()),
        )
        .organization(Some(organization))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .expect("the added member opens the organization");

    admin.tend().await.unwrap();
    assert_eq!(
        admin.unsigned_members(),
        [own.account()],
        "the invited member is signed in; the one the server added is not"
    );
    invited.tend().await.unwrap();
    assert!(invited.unsigned_members().is_empty());
    added.tend().await.unwrap();
    assert_eq!(
        added.unsigned_members(),
        [own.account()],
        "a member signed in by no one is told it waits"
    );
    admin.tend().await.unwrap();
    assert!(
        admin
            .checked_organization()
            .await
            .unwrap()
            .unwrap()
            .member(own.account().into_bytes())
            .is_none(),
        "an administrator's device signed in a member it never verified"
    );

    harness.stop().await;
}
