use super::*;
use crate::store::passkeys::MAX_PASSKEYS;

#[tokio::test]
async fn an_account_holds_a_bounded_number_of_passkeys() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let mut labels = std::collections::HashSet::new();
    for _ in 0..MAX_PASSKEYS {
        ada.add_passkey(&harness).await;
    }
    // Added the same minute, each is still told apart.
    for identity in page_of(&ada.get(&harness, "/auth/settings").await).identities {
        assert!(labels.insert(identity.label));
    }
    let origin = ada.origin();
    let (status, response) = ada
        .passkey(&harness, "/auth/settings", |options| {
            crate::fake_authenticator::FakeAuthenticator::create(options, &origin, None).1
        })
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(response.contains("the most it may have"), "{response}");
}
