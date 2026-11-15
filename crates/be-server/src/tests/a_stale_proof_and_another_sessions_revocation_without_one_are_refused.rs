use super::*;

#[tokio::test]
async fn a_stale_proof_and_another_sessions_revocation_without_one_are_refused() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut other = Browser::default();
    other.sign_in_by_code(&harness, "owner@example.com").await;
    let mut browser = Browser::default();
    browser.sign_in_by_code(&harness, "owner@example.com").await;
    // Signing in by code is a proof; these tests need it gone stale.
    harness.stale_proofs();
    let store = ServerStore::open(&harness.directory).unwrap();
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    let sessions = store
        .account_sessions(profile.account, &browser.cookie)
        .await
        .unwrap();
    let theirs = sessions
        .iter()
        .find(|session| !session.current && session.kind == "browser")
        .unwrap()
        .id
        .clone();
    // Signing out another session needs a fresh proof.
    let fields = [("session", theirs.as_str())];
    assert_eq!(
        browser
            .submit(&harness, "/auth/sessions", "/auth/sessions/revoke", &fields)
            .await
            .0,
        400
    );
    assert!(store.browser_profile(&other.cookie).await.is_ok());
    // A proof gone stale is no proof.
    browser.prove(&harness, "owner@example.com").await;
    let database = rusqlite::Connection::open(harness.directory.join("metadata.sqlite")).unwrap();
    database
        .execute(
            "UPDATE sessions SET proved = proved - ?1 - 1 WHERE proved > 0",
            [crate::store::codes::FRESH_PROOF],
        )
        .unwrap();
    assert_eq!(
        browser
            .submit(&harness, "/auth/sessions", "/auth/sessions/revoke", &fields)
            .await
            .0,
        400
    );
    browser.prove(&harness, "owner@example.com").await;
    assert_eq!(
        browser
            .submit(&harness, "/auth/sessions", "/auth/sessions/revoke", &fields)
            .await
            .0,
        303
    );
    assert!(store.browser_profile(&other.cookie).await.is_err());
    // Its own session it signs out without one.
    let mine = store
        .account_sessions(profile.account, &browser.cookie)
        .await
        .unwrap()
        .into_iter()
        .find(|session| session.current)
        .unwrap()
        .id;
    let mut unproved = Browser::default();
    unproved
        .sign_in_by_code(&harness, "owner@example.com")
        .await;
    harness.stale_proofs();
    let own = store
        .account_sessions(profile.account, &unproved.cookie)
        .await
        .unwrap()
        .into_iter()
        .find(|session| session.current)
        .unwrap()
        .id;
    assert_ne!(own, mine);
    assert_eq!(
        unproved
            .submit(
                &harness,
                "/auth/sessions",
                "/auth/sessions/revoke",
                &[("session", &own)]
            )
            .await
            .0,
        303
    );
    assert!(store.browser_profile(&unproved.cookie).await.is_err());
}
