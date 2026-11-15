use super::*;

#[tokio::test]
async fn an_account_with_no_email_is_told_of_its_identities_on_its_devices() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    ada.add_passkey(&harness).await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let account = store.browser_profile(&ada.cookie).await.unwrap().account;
    // Its email goes, so the account has none: what it is told next comes to its devices.
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
    let mailed = harness.mails_to("ada@example.com");
    let mut device = harness.client().await;
    let token = store.issue_session(account).await.unwrap();
    let signed_in = device
        .call(|peer| client::account::authenticate(peer, Some(token)))
        .await;
    assert!(signed_in.is_ok(), "{signed_in:?}");
    let listed = device
        .call(|peer| client::account::account_notices(peer))
        .await
        .unwrap();
    assert!(listed.is_empty(), "{listed:?}");
    // A second passkey, then the first removed: each told on its devices, and no mail.
    ada.add_passkey(&harness).await;
    let id = page_of(&ada.get(&harness, "/auth/settings").await)
        .identities
        .into_iter()
        .find(|identity| identity.kind == "passkey")
        .unwrap()
        .id;
    let (status, response) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &id)],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(harness.mails_to("ada@example.com"), mailed);
    let listed = device
        .call(|peer| client::account::account_notices(peer))
        .await
        .unwrap();
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert!(listed[0].text.contains("can now sign in"), "{listed:?}");
    assert!(listed[1].text.contains("can no longer sign in"), "{listed:?}");
    // Dismissed on one device, it is gone for all; another account's is left alone.
    let dismissed = listed[0].id;
    let mut other = harness.member("other@example.com").await;
    let _ = other
        .call(|peer| client::account::dismiss_account_notice(peer, dismissed + 1))
        .await;
    let done = device
        .call(|peer| client::account::dismiss_account_notice(peer, dismissed))
        .await;
    assert!(matches!(done, Ok(())), "{done:?}");
    let listed = device
        .call(|peer| client::account::account_notices(peer))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
}
