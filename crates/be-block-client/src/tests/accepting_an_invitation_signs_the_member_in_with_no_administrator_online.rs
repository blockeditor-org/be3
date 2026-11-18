use super::*;

/// Accepting an invitation signs the accepter into the organization's log with the key its code
/// derives, so no administrator's device need be online. The invitation signs in no one else,
/// even where the server forgets it was used, and a member record the server makes up itself,
/// signed with no invitation's key, leaves the log refused by whoever is served it whole.
#[tokio::test]
async fn accepting_an_invitation_signs_the_member_in_with_no_administrator_online() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let organization = admin.organization();
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
        organization,
        &crate::contacts::Pinned {
            root: admin.keys().organization_root(),
            escrow: admin.keys().escrow(),
            made: 0,
        },
        &admin.checked_organization().await.unwrap().unwrap(),
    )
    .await
    .unwrap();
    let admin_token = admin.token().to_owned();
    let admin_identity = admin.keys().identity().clone();
    drop(admin);

    let own = harness.owner("member@example.com").await;
    let accept = |peer: &Peer<MemoryStore>, email: &'static str| {
        let theirs = crate::contacts::card(peer.account(), peer.keys().identity(), email, email);
        let connection = peer.connection();
        let identity = peer.keys().identity().clone();
        let code = code.clone();
        async move {
            crate::contacts::accept_link(
                &connection,
                &identity,
                &theirs,
                &code,
                crate::contacts::Expected::Invitation,
            )
            .await
        }
    };
    accept(&own, "member@example.com").await.unwrap();
    let member = Peer::connect(
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
    .unwrap();
    let checked = member.checked_organization().await.unwrap().unwrap();
    assert_eq!(
        checked.member(own.account().into_bytes()),
        Some(own.keys().identity().identity()),
        "accepting signed the member in, with no administrator online"
    );

    // Used, the invitation signs in no second account: not while the server keeps that it was
    // used, nor once it forgets, as the log holds its key spent.
    let second = harness.owner("second@example.com").await;
    assert!(accept(&second, "second@example.com").await.is_err());
    harness
        .database()
        .execute(
            "UPDATE links SET accepter_id = '', invite_ended = 0 WHERE invite_organization = ?1",
            [organization.to_string()],
        )
        .unwrap();
    let again = accept(&second, "second@example.com").await;
    assert!(
        again.is_err(),
        "a used invitation signed in a second account: {again:?}"
    );

    // A member the server signs in itself, claiming an invitation's key without its code, is
    // caught by whoever reads the whole log, though the invitation is open and unused and the
    // keys it names endorse the record: only the code's key signs a person in.
    let admin = Peer::connect(
        PeerConfig::new(&harness.url, admin_identity, Credentials::Token(admin_token))
            .organization(Some(organization))
            .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let fresh = crate::contacts::invite(
        &admin.connection(),
        admin.keys().identity(),
        &card,
        organization,
        &crate::contacts::Pinned {
            root: admin.keys().organization_root(),
            escrow: admin.keys().escrow(),
            made: 0,
        },
        &admin.checked_organization().await.unwrap().unwrap(),
    )
    .await
    .unwrap();
    let head = admin.checked_organization().await.unwrap().unwrap().head;
    let endorsed = be_block_keys::Entry::accepted(
        &fresh,
        organization.into_bytes(),
        head,
        second.account().into_bytes(),
        second.keys().identity(),
    );
    let forged = be_block_keys::Entry::sign(
        second.keys().identity(),
        second.account().into_bytes(),
        organization.into_bytes(),
        Some(head),
        false,
        endorsed.record,
    );
    let database = harness.database();
    let position: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM organization_log WHERE organization_id = ?1",
            [organization.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO organization_log (organization_id, position, entry) VALUES (?1, ?2, ?3)",
            rusqlite::params![organization.to_string(), position, forged.encode()],
        )
        .unwrap();
    // Served the whole log, as is anyone who seals (a member below Edit is served a path that
    // hides other members' records).
    let read = admin.checked_organization().await;
    assert!(
        matches!(read, Err(ClientError::Forged(of)) if of == organization),
        "the administrator took a log the server signed a member into: {read:?}"
    );
    harness.stop().await;
}
