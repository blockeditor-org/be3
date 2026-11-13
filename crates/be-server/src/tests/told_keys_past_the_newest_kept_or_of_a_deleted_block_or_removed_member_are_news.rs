use super::*;

#[tokio::test]
async fn told_keys_past_the_newest_kept_or_of_a_deleted_block_or_removed_member_are_news() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let identity = Identity {
        account,
        organization,
        role: OrganizationRole::Administrator,
        bound: None,
    };
    let block = Uuid::from_u128(1);
    let store = ServerStore::open(&harness.directory).unwrap();
    let key = |n: usize| be_block_protocol::told_key(block, &n.to_string());
    let tell = |keys: Vec<Hash>| {
        let store = store.clone();
        async move { store.tell(identity, block, keys).await.unwrap() }
    };

    let max = be_block_protocol::MAX_TOLD_KEYS;
    assert!(tell((0..max).map(key).collect()).await);
    assert!(!tell(vec![key(0)]).await, "every kept key is told");
    assert!(tell(vec![key(max)]).await);
    assert!(
        tell(vec![key(0)]).await,
        "the oldest past the newest kept is news again"
    );
    assert!(!tell(vec![key(max)]).await);

    let forgotten = store.clone();
    forgotten
        .with_database(move |_, database| {
            crate::notifications::forget(database, organization, block)?;
            Ok(())
        })
        .await
        .unwrap();
    assert!(
        tell(vec![key(max)]).await,
        "a deleted block's keys are forgotten"
    );

    let forgotten = store.clone();
    forgotten
        .with_database(move |_, database| {
            crate::notifications::forget_member(database, organization, account)?;
            Ok(())
        })
        .await
        .unwrap();
    assert!(
        tell(vec![key(max)]).await,
        "a removed member's keys are forgotten"
    );

    harness.stop().await;
}
