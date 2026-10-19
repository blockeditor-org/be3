use super::*;
#[tokio::test]
async fn legacy_local_profile_sessions_remain_usable() {
    let directory = std::env::temp_dir().join(format!("legacy-sessions-{}", Uuid::new_v4()));
    let store = ServerStore::open(&directory).unwrap();
    let (local, local_token) = store.create_local_profile("Local").await.unwrap();
    let (_, remote_token) = store
        .register("remote@example.com", "Remote", "a good password")
        .await
        .unwrap();
    let database = rusqlite::Connection::open(directory.join("metadata.sqlite")).unwrap();
    database
        .execute(
            "UPDATE sessions SET kind = 'legacy', public_id = '', created = 0",
            [],
        )
        .unwrap();
    drop((store, database));
    let store = ServerStore::open(&directory).unwrap();
    assert_eq!(
        store.resolve_token(&local_token).await.unwrap().account,
        local.account
    );
    assert!(store.resolve_token(&remote_token).await.is_err());
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}
