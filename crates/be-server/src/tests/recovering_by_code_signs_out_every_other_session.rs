use super::*;

#[tokio::test]
async fn recovering_by_code_signs_out_every_other_session() {
    let harness = Harness::configured(ServerConfig {
        sign_in_poll_interval: Duration::ZERO,
        ..ServerConfig::OPEN
    })
    .await;
    harness.member("owner@example.com").await;
    let mut first = Browser::default();
    first.sign_in_by_code(&harness, "owner@example.com").await;
    // The first browser approves a device that has not claimed its sign-in yet.
    let (device, secret) = begin_session(&harness).await;
    let (status, _) = first
        .submit(
            &harness,
            &format!("/auth?code={device}"),
            "/auth/approve",
            &[("code", &device)],
        )
        .await;
    assert_eq!(status, 303);
    let mut recovering = Browser::default();
    let (status, _) = recovering
        .submit(
            &harness,
            "/auth/recover",
            "/auth/code/send",
            &[("email", "owner@example.com"), ("purpose", "recover")],
        )
        .await;
    assert_eq!(status, 303);
    let (code, _) = harness.mailed_code("owner@example.com");
    assert_eq!(recovering.enter_code(&harness, &code).await.0, 303);
    let store = ServerStore::open(&harness.directory).unwrap();
    assert!(store.browser_profile(&first.cookie).await.is_err());
    // The approval went with the session that made it.
    let claimed = poll_login(&harness, &device, &secret).await;
    assert!(
        !matches!(
            claimed,
            Ok(be_block_protocol::BrowserLoginState::Approved(_))
        ),
        "an approval made before recovering still signed in: {claimed:?}"
    );
    let profile = store.browser_profile(&recovering.cookie).await.unwrap();
    assert!(
        store.is_fresh(&recovering.cookie).await.unwrap(),
        "recovering counts as fresh proof"
    );
    assert_eq!(
        store
            .account_sessions(profile.account, &recovering.cookie)
            .await
            .unwrap()
            .len(),
        1
    );
}
