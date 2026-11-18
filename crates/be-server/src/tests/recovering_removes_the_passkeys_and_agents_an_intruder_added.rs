use super::*;

/// An intruder who held the account for a while adds a passkey and an agent; once the owner
/// recovers it by email, neither is a way back in (guides/blocks/server.md, "Accounts").
#[tokio::test]
async fn recovering_removes_the_passkeys_and_agents_an_intruder_added() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let mut intruder = Browser::at_localhost(&harness);
    intruder
        .sign_in_by_code(&harness, "owner@example.com")
        .await;
    intruder.add_passkey(&harness).await;
    let account = store
        .browser_profile(&intruder.cookie)
        .await
        .unwrap()
        .account;
    let organization = store.personal_organization().await.unwrap();
    let (_, agent_token, _) = store
        .create_agent(
            account,
            organization,
            agent_id(account, &unused_keys(7)),
            "Helper",
            [7; 32],
            [7; 32],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            None,
        )
        .await
        .unwrap();
    assert!(store.resolve_token(&agent_token).await.is_ok());

    let mut owner = Browser::at_localhost(&harness);
    let (status, _) = owner
        .submit(
            &harness,
            "/auth/recover",
            "/auth/code/send",
            &[("email", "owner@example.com"), ("purpose", "recover")],
        )
        .await;
    assert_eq!(status, 303);
    let (code, _) = harness.mailed_code("owner@example.com");
    assert_eq!(owner.enter_code(&harness, &code).await.0, 303);

    let kinds: Vec<String> = store
        .identities(account)
        .await
        .unwrap()
        .into_iter()
        .map(|identity| identity.kind)
        .collect();
    assert_eq!(kinds, ["email"], "recovering kept another way in");
    assert!(
        store.resolve_token(&agent_token).await.is_err(),
        "the intruder's agent still signs in after recovering"
    );
    // The owner hears what went, once, with both named.
    let told: Vec<String> = harness
        .mail
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|message| {
            message.recipient == "owner@example.com"
                && message.subject == "Recovering your be3 account removed ways in"
        })
        .map(|message| message.body.clone())
        .collect();
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(
        told[0].contains("Passkey") && told[0].contains("the agent Helper"),
        "{told:?}"
    );
    let history = page_of(&owner.get(&harness, "/auth/settings").await).identity_changes;
    assert!(
        history.iter().any(|change| change.text.contains("Passkey")),
        "the settings page lists the passkey as removed: {history:?}"
    );
}
