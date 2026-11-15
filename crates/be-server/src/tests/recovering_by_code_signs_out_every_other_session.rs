use super::*;

#[tokio::test]
async fn recovering_by_code_signs_out_every_other_session() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut first = Browser::default();
    first.sign_in_by_code(&harness, "owner@example.com").await;
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
