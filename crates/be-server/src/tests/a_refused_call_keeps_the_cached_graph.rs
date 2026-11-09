use super::*;

#[tokio::test]
async fn a_refused_call_keeps_the_cached_graph() {
    let directory = std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4()));
    let store = ServerStore::open(&directory).unwrap();
    let (profile, _) = store.create_local_profile("Reader").await.unwrap();
    let identity = Identity {
        account: profile.account,
        organization: store.personal_organization().await.unwrap(),
        role: OrganizationRole::Editor,
        bound: None,
    };

    for _ in 0..3 {
        let read = store.read_block(identity, Uuid::new_v4()).await;
        assert!(matches!(read, Err(ServerError::Refused(..))), "{read:?}");
    }
    assert_eq!(store.graph_loads(), 1, "a refused read reloaded the graph");

    let _ = std::fs::remove_dir_all(directory);
}
