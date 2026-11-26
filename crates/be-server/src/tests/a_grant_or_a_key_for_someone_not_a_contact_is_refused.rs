use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_grant_or_a_key_for_someone_not_a_contact_is_refused() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    owner.personal().await;
    let mut stranger = harness.member("stranger@example.com").await;
    let stranger_id = stranger.id();
    stranger.personal().await;
    let shared = owner.scope_root(owner_id, BlockParent::Root).await;

    let (granted, keyed) = attempts(&mut owner, &shared, stranger_id).await;
    assert!(refused_as(&granted, ErrorCode::NotAContact), "{granted:?}");
    assert!(refused_as(&keyed, ErrorCode::NotAContact), "{keyed:?}");
    let listed = owner
        .call(|peer| {
            peer.read::<be_block_protocol::AccessList>(be_block_protocol::Query::Access(
                shared.block,
            ))
        })
        .await;
    let Ok(list) = listed else {
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

    let link = owner.link().await;
    let accepted = stranger.accept(link, &[]).await;
    assert!(matches!(accepted, Ok(())), "{accepted:?}");
    let (granted, keyed) = attempts(&mut owner, &shared, stranger_id).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    assert!(matches!(keyed, Ok(())), "{keyed:?}");

    harness.stop().await;
}

async fn attempts(
    owner: &mut TestClient,
    shared: &Scoped,
    stranger: Uuid,
) -> (Result<(), Error>, Result<(), Error>) {
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
            .call(|peer| {
                peer.write::<()>(be_block_protocol::Op::PutKeys(
                    be_block_protocol::PutKeys::Rows(keys),
                ))
            })
            .await,
    )
}
