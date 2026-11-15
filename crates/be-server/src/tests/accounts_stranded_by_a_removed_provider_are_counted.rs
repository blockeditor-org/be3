use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn accounts_stranded_by_a_removed_provider_are_counted() {
    let (harness, provider) = with_provider(false).await;
    harness.member("ada@example.com").await;
    let mut browser = Browser::default();
    browser
        .provider_sign_in(&harness, &provider, "test", "", Grant::new("only"))
        .await;
    browser
        .submit(
            &harness,
            "/auth/oidc/register",
            "/auth/oidc/register",
            &[("invite", &harness.invite()), ("display_name", "Only")],
        )
        .await;
    let store = ServerStore::open(&harness.directory).unwrap();
    assert_eq!(
        store
            .stranded_accounts(vec![provider.issuer.clone()], None)
            .await
            .unwrap(),
        0
    );
    // Without the provider, the account that signs in only with it has no way in.
    assert_eq!(store.stranded_accounts(Vec::new(), None).await.unwrap(), 1);
}
