use super::*;
use be_graph::Access;

#[tokio::test]
async fn a_grant_or_a_key_for_someone_not_a_contact_is_refused() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    owner.personal().await;
    let mut stranger = harness.client().await;
    let stranger_id = stranger.register("stranger@example.com").await;
    stranger.personal().await;
    let shared = owner.scope_root(owner_id, BlockParent::Root).await;

    let (granted, keyed) = attempts(&mut owner, &shared, stranger_id).await;
    assert!(refused_as(&granted, ErrorCode::NotAContact), "{granted:?}");
    assert!(refused_as(&keyed, ErrorCode::NotAContact), "{keyed:?}");
    let listed = owner
        .send(|request| ClientMessage::ListAccess {
            request,
            block: shared.block,
        })
        .await;
    let ServerMessage::AccessList { list, .. } = listed else {
        panic!("listing access failed: {listed:?}");
    };
    assert_eq!(
        list.entries
            .iter()
            .map(|entry| entry.account)
            .collect::<Vec<_>>(),
        vec![owner_id],
        "the share list showed a member who is neither a grantee nor a contact"
    );

    let link = owner.link(None).await;
    let accepted = stranger.accept(link, &[]).await;
    assert!(matches!(accepted, ServerMessage::Ok { .. }), "{accepted:?}");
    let (granted, keyed) = attempts(&mut owner, &shared, stranger_id).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    assert!(matches!(keyed, ServerMessage::Ok { .. }), "{keyed:?}");

    harness.stop().await;
}

async fn attempts(
    owner: &mut TestClient,
    shared: &Scoped,
    stranger: Uuid,
) -> (ServerMessage, ServerMessage) {
    let keys = KeySet {
        keys: Vec::new(),
        wrapped: vec![wrap(
            shared.scope,
            WrappedBy::Account(stranger),
            b"the scope key, sealed to someone",
        )],
    };
    (
        owner.grant(shared.block, stranger, Access::View).await,
        owner
            .send(|request| ClientMessage::PutKeys { request, keys })
            .await,
    )
}
