use super::*;

#[tokio::test]
async fn an_invited_account_joins_the_organization_and_a_logout_ends_the_session() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("shared").await;
    let mut guest = harness.member("Guest@Example.com").await;

    let refused = guest
        .call(|peer| {
            peer.read_in::<be_block_protocol::Listed>(
                Some(organization),
                Query::List {
                    listing: Listing::Children(BlockParent::Root),
                    after: None,
                },
            )
        })
        .await;
    assert!(matches!(refused, Err(_)), "{refused:?}");

    let invitation = Uuid::from_u128(7);
    let response = owner
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::CreateLink {
                link: invitation,
                card: b"sealed to the invite code".to_vec(),
                code: Vec::new(),
                kind: be_block_protocol::LinkKind::Invitation(be_block_protocol::Invitation {
                    organization,
                    escrow: Vec::new(),
                }),
            })
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let fetched = guest
        .call(|peer| {
            peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(invitation))
        })
        .await;
    let Ok(found) = fetched else {
        panic!("reading the invitation failed: {fetched:?}");
    };
    assert_eq!(found.organization_name, "shared");
    assert_eq!(found.card, b"sealed to the invite code");

    let response = guest
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::AcceptLink {
                link: invitation,
                card: Vec::new(),
                contact: Vec::new(),
                key: Vec::new(),
            })
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    guest.open(organization).await;
    let again = guest
        .call(|peer| {
            peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(invitation))
        })
        .await;
    assert!(
        matches!(again, Err(_)),
        "the invitation outlived its use: {again:?}"
    );

    let token = guest.token.clone();
    let response = guest
        .call(|peer| peer.write::<()>(be_block_protocol::Op::RevokeToken(None)))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let response = harness.sign_in(&token).await;
    assert!(response.is_err(), "a token logged out still signs in");

    harness.stop().await;
}
