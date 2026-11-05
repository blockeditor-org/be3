use super::*;
use crate::sessions::MAX_EMPTY_SESSIONS;

#[tokio::test]
async fn the_session_registry_forgets_the_longest_empty_sessions_first() {
    let registry = SessionRegistry::new(Duration::ZERO);
    let client = 7;
    let clean = CommitId::from_hash(Hash::of(b"clean"));
    let blocks: Vec<crate::watch::Place> = (0..=MAX_EMPTY_SESSIONS)
        .map(|_| crate::watch::Place {
            organization: Uuid::nil(),
            block: Uuid::new_v4(),
        })
        .collect();
    for block in &blocks {
        let state = registry.join(*block, client).await;
        registry
            .heartbeat(*block, client, state.generation, Some(clean))
            .await
            .expect("the owner's heartbeat is taken");
        assert_eq!(registry.blocks_of(client).await, vec![*block]);
        registry.leave(*block, client).await;
    }
    assert_eq!(registry.held().await, MAX_EMPTY_SESSIONS);
    assert_eq!(registry.state(blocks[0]).await.clean_at, None);
    assert_eq!(registry.state(blocks[1]).await.clean_at, Some(clean));

    registry.join(blocks[1], client).await;
    registry.join(blocks[2], client).await;
    let left = registry.leave_all(client).await;
    assert_eq!(left.len(), 2);
    assert!(registry.blocks_of(client).await.is_empty());
}
