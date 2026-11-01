use super::*;

#[tokio::test]
async fn an_invited_account_joins_the_organization_and_a_logout_ends_the_session() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("shared").await;
    let mut guest = harness.member("Guest@Example.com").await;

    let refused = guest
        .call(|peer| client::organizations::open_organization(peer, organization))
        .await;
    assert!(matches!(refused, Err(_)), "{refused:?}");

    let invitation = Uuid::from_u128(7);
    let response = owner
        .call(|peer| {
            client::organizations::invite(
                peer,
                invitation,
                organization,
                "guest@example.com".into(),
                OrganizationRole::Editor,
                b"sealed to the invite code".to_vec(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let mut outsider = harness.member("outsider@example.com").await;
    for refused in [
        outsider
            .call(|peer| client::organizations::get_invitation(peer, invitation))
            .await
            .map(|_| ()),
        outsider
            .call(|peer| {
                client::organizations::accept_invitation(
                    peer,
                    invitation,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            })
            .await,
    ] {
        assert!(
            matches!(refused, Err(_)),
            "an account with another email used the invitation: {refused:?}"
        );
    }

    let fetched = guest
        .call(|peer| client::organizations::get_invitation(peer, invitation))
        .await;
    let Ok(found) = fetched else {
        panic!("reading the invitation failed: {fetched:?}");
    };
    assert_eq!(found.organization_name, "shared");
    assert_eq!(found.sealed, b"sealed to the invite code");

    let response = guest
        .call(|peer| {
            client::organizations::accept_invitation(
                peer,
                invitation,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    guest.open(organization).await;
    let again = guest
        .call(|peer| client::organizations::get_invitation(peer, invitation))
        .await;
    assert!(
        matches!(again, Err(_)),
        "the invitation outlived its use: {again:?}"
    );

    let token = guest.token.clone();
    let response = guest.call(|peer| client::account::logout(peer)).await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let mut returning = harness.client().await;
    let response = returning
        .call(|peer| client::account::authenticate(peer, Some(token)))
        .await;
    assert!(matches!(response, Err(_)), "{response:?}");

    harness.stop().await;
}
