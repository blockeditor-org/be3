use super::*;
use be_graph::Access;

#[tokio::test]
async fn a_pending_grant_is_applied_when_its_link_is_accepted() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let personal = owner.personal().await;
    let shared = owner.scope_root(owner_id, BlockParent::Root).await;
    let other_root = owner.scope_root(owner_id, BlockParent::Root).await;

    let pending = |key| be_protocol::PendingGrant {
        organization: personal,
        block: shared.block,
        access: Access::View,
        key,
        sealed: b"the scope key, sealed with the code".to_vec(),
    };
    let wrong_key = owner
        .call(|peer| {
            client::contacts::create_contact_link(
                peer,
                Uuid::new_v4(),
                Vec::new(),
                Vec::new(),
                Some(pending(other_root.scope)),
            )
        })
        .await;
    assert!(
        matches!(wrong_key, Err(_)),
        "a link carried another block's key: {wrong_key:?}"
    );
    let link = owner.link(Some(pending(shared.scope))).await;

    let mut other = harness.client().await;
    let other_id = other.register("other@example.com").await;
    other.personal().await;
    let hidden = other
        .call(|peer| client::blocks::read_block(peer, shared.block))
        .await;
    assert!(matches!(hidden, Err(_)), "{hidden:?}");

    let accepted = other
        .accept(link, b"the scope key, sealed to the accepter")
        .await;
    assert!(matches!(accepted, Ok(())), "{accepted:?}");
    assert_eq!(other.read_block(shared.block).await.access, Access::View);
    let keys = other.keys(Some(vec![shared.block])).await;
    assert!(
        keys.wrapped
            .iter()
            .any(|wrapped| wrapped.key == shared.scope
                && wrapped.wrapped_by == WrappedBy::Account(other_id)
                && wrapped.sealed == b"the scope key, sealed to the accepter"),
        "the accepter was not given the scope key: {keys:?}"
    );
    assert!(
        keys.wrapped.iter().any(|wrapped| wrapped.key == shared.data
            && wrapped.wrapped_by == WrappedBy::Key(shared.scope)),
        "{keys:?}"
    );

    harness.stop().await;
}
