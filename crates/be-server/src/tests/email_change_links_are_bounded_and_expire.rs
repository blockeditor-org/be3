use super::*;
use crate::store::portal::{MAX_MAIL_LINKS_PER_ADDRESS, MAX_MAIL_LINKS_PER_EMAIL};

#[tokio::test]
async fn email_change_links_are_bounded_and_expire() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let (profile, _) = store
        .register("expire@example.com", "Expiry")
        .await
        .unwrap();
    let browser = store.browser_session(profile.account).await.unwrap();
    prove(&store, profile.account, "expire@example.com", &browser).await;
    let change = |email: String, address: &'static str| {
        let (store, browser) = (&store, &browser);
        async move {
            store
                .email_change_link(profile.account, &email, browser, address)
                .await
        }
    };
    let first = change("new@example.com".into(), "192.0.2.1").await.unwrap();
    for _ in 1..MAX_MAIL_LINKS_PER_EMAIL {
        change("new@example.com".into(), "192.0.2.1").await.unwrap();
    }
    // One new email takes no more than its share.
    assert!(
        change("new@example.com".into(), "198.51.100.7")
            .await
            .is_err()
    );
    // Nor does one address.
    for index in MAX_MAIL_LINKS_PER_EMAIL..MAX_MAIL_LINKS_PER_ADDRESS {
        change(format!("other{index}@example.com"), "192.0.2.1")
            .await
            .unwrap();
    }
    assert!(
        change("last@example.com".into(), "192.0.2.1")
            .await
            .is_err()
    );
    assert!(
        change("last@example.com".into(), "198.51.100.7")
            .await
            .is_ok()
    );
    let database = metadata_database(&harness.directory);
    database
        .execute("UPDATE account_mail SET expires = 0", [])
        .unwrap();
    database
        .execute("UPDATE sessions SET expires = 0 WHERE kind = 'browser'", [])
        .unwrap();
    assert!(store.confirm_mail(&first.token, profile.account).await.is_err());
    assert!(store.browser_profile(&browser).await.is_err());
}
