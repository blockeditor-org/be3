use super::*;
use crate::store::APP_SESSION_IDLE;

/// An app token outlives no browser session for good: left unused for `APP_SESSION_IDLE` it
/// stops working, and each use moves that day on (guides/blocks/server.md, "Accounts").
#[tokio::test]
async fn an_app_token_left_unused_expires_and_one_in_use_is_renewed() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let (_, unused) = store
        .register("unused@example.com", "Unused")
        .await
        .unwrap();
    let (_, used) = store.register("used@example.com", "Used").await.unwrap();
    let expires = |token: &str| -> i64 {
        metadata_database(&harness.directory)
            .query_row(
                "SELECT expires FROM sessions WHERE token_hash = ?1",
                [crate::store::hash_token(token)],
                |row| row.get(0),
            )
            .unwrap()
    };
    let now = crate::store::portal::now();
    assert!(expires(&unused) >= now + APP_SESSION_IDLE - 60);
    // Ninety days go by for both; one was last used just before they ran out.
    metadata_database(&harness.directory)
        .execute("UPDATE sessions SET expires = ?1", [now - 1])
        .unwrap();
    assert!(store.resolve_token(&unused).await.is_err());
    metadata_database(&harness.directory)
        .execute(
            "UPDATE sessions SET expires = ?1 WHERE token_hash = ?2",
            rusqlite::params![now + 60, crate::store::hash_token(&used)],
        )
        .unwrap();
    assert!(store.resolve_token(&used).await.is_ok());
    assert!(
        expires(&used) >= now + APP_SESSION_IDLE - 60,
        "a use renews the token"
    );
}
