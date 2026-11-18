use super::*;
use crate::store::login_links::{self, LinkFor};

#[tokio::test]
async fn a_login_link_for_a_new_email_makes_its_account() {
    // A closed server: the link needs neither registration nor an invite code, nor the log.
    let harness = Harness::configured(ServerConfig::default()).await;
    let link =
        login_links::create(&harness.directory, LinkFor::Email("first@example.com")).unwrap();
    assert!(link.registers);
    let mut browser = Browser::default();
    let (status, response) = browser
        .submit(
            &harness,
            &login_links::path(&link.token),
            "/auth/link",
            &[("token", &link.token)],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    let store = ServerStore::open(&harness.directory).unwrap();
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    assert_eq!(profile.email, "first@example.com");
    assert_eq!(profile.display_name, "first");
    let identities = store.identities(profile.account).await.unwrap();
    assert_eq!(
        identities
            .iter()
            .map(|identity| (identity.kind.as_str(), identity.subject.as_str()))
            .collect::<Vec<_>>(),
        [("email", "first@example.com")]
    );
    assert!(harness.mail.0.lock().unwrap().is_empty());
    // An agent's address has an account but no identity: no link makes another account for it.
    metadata_database(&harness.directory)
        .execute(
            "INSERT INTO accounts (id, email, display_name) VALUES (?1, 'agent@agents.invalid', 'Agent')",
            [Uuid::new_v4().to_string()],
        )
        .unwrap();
    assert!(
        login_links::create(&harness.directory, LinkFor::Email("agent@agents.invalid")).is_err()
    );
}
