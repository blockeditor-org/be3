use super::*;

#[tokio::test]
async fn a_link_or_a_proof_begins_only_with_the_settings_pages_check() {
    let (harness, _provider) = with_provider(false).await;
    harness.member("ada@example.com").await;
    harness.member("eve@example.com").await;
    let mut ada = Browser::default();
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let mut eve = Browser::default();
    eve.sign_in_by_code(&harness, "eve@example.com").await;
    // A page elsewhere can link ada's browser to a start, but not with ada's check: none, a
    // guess, and another session's are all refused.
    let eves = format!(
        "&check={}",
        crate::store::providers::start_check(&eve.cookie)
    );
    for intent in ["link", "proof"] {
        for check in ["", "&check=guess", eves.as_str()] {
            let (status, _) = ada
                .request(
                    &harness,
                    "GET",
                    &format!("/auth/oidc/test/start?intent={intent}{check}"),
                    "",
                )
                .await;
            assert_eq!(status, 400, "{intent}{check}");
        }
    }
    let begun: i64 = metadata_database(&harness.directory)
        .query_row("SELECT count(*) FROM oidc_states", [], |row| row.get(0))
        .unwrap();
    assert_eq!(begun, 0);
    // The settings page's own links carry it, and begin.
    let page = page_of(&ada.get(&harness, "/auth/settings").await);
    let (status, response) = ada
        .request(&harness, "GET", &page.providers[0].href, "")
        .await;
    assert_eq!(status, 303, "{response}");
}
