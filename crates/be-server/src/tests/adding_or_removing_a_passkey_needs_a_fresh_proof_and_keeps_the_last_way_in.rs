use super::*;

#[tokio::test]
async fn adding_or_removing_a_passkey_needs_a_fresh_proof_and_keeps_the_last_way_in() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    // A page that offered to add one while the proof was fresh adds nothing once it is stale.
    let page = page_of(&ada.get(&harness, "/auth/settings").await);
    let offer = page.passkey.clone().unwrap();
    assert_eq!(offer.step, be_server_portal::PasskeyStep::Add);
    harness.stale_proofs();
    let (_, answer) =
        crate::fake_authenticator::FakeAuthenticator::create(&offer.options, &ada.origin(), None);
    let body = format!(
        "csrf={}&credential={}",
        portal::encode(&page.csrf),
        portal::encode(&answer)
    );
    let (status, response) = ada
        .request(&harness, "POST", "/auth/passkey/add", &body)
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(response.contains("Confirm it is you first"), "{response}");
    // A stale page offers no passkey to add while the account has none to prove with.
    assert_eq!(
        page_of(&ada.get(&harness, "/auth/settings").await).passkey,
        None
    );
    ada.prove(&harness, "ada@example.com").await;
    let mut authenticator = ada.add_passkey(&harness).await;
    // Once stale again, the page offers to prove with it, and removing it is refused until then.
    harness.stale_proofs();
    let passkey = page_of(&ada.get(&harness, "/auth/settings").await)
        .identities
        .into_iter()
        .find(|identity| identity.kind == "passkey")
        .unwrap();
    let id = passkey.id;
    let (status, _) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &id)],
        )
        .await;
    assert_eq!(status, 400);
    let origin = ada.origin();
    let (status, response) = ada
        .passkey(&harness, "/auth/settings", |options| {
            authenticator.get(options, &origin)
        })
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(location(&response), "/auth/settings?proved=1");
    // The email goes, with its notice, and the passkey is the last way in: it can't go.
    let email = page_of(&ada.get(&harness, "/auth/settings").await)
        .identities
        .into_iter()
        .find(|identity| identity.kind == "email")
        .unwrap();
    let (status, response) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &email.id)],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    let identities = page_of(&ada.get(&harness, "/auth/settings").await).identities;
    assert_eq!(identities.len(), 1);
    assert!(!identities[0].removable);
    // Its passkey works only at this RP ID: were `--auth-origin` to change, nothing lets it in
    // but a login link.
    let store = ServerStore::open(&harness.directory).unwrap();
    for (rp_id, stranded) in [
        (Some("localhost"), 0),
        (Some("elsewhere.example"), 1),
        (None, 1),
    ] {
        assert_eq!(
            store
                .stranded_accounts(Vec::new(), rp_id.map(str::to_owned))
                .await
                .unwrap(),
            stranded,
            "{rp_id:?}"
        );
    }
    let (status, response) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &id)],
        )
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(response.contains("only way to sign in"), "{response}");
    // A second passkey, and then the first can go, with a notice, and no longer signs in.
    let second = ada.add_passkey(&harness).await;
    assert_ne!(second.credential_id, authenticator.credential_id);
    let (status, response) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &id)],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    let mut elsewhere = Browser::at_localhost(&harness);
    let (status, response) = elsewhere
        .sign_in_with_passkey(&harness, &mut authenticator)
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(elsewhere.cookie.is_empty());
}
