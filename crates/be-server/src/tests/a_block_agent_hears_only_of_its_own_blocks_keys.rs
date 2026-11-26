use super::*;
use be_block_graph::Access;

/// A block's agent follows the keys topic, so a key sealed to it after it connected reaches it,
/// and hears only of its own block's keys: one stored for a block outside it, though granted
/// there too, tells it nothing, whether sealed to it or a new key wrapped under the block's
/// scope, which tells every viewer of the block.
#[tokio::test]
async fn a_block_agent_hears_only_of_its_own_blocks_keys() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    let organization = owner.organization("keys").await;
    let own = owner.scope_root(owner_id, BlockParent::Root).await;
    let outside = owner.scope_root(owner_id, BlockParent::Root).await;
    let (account, token) = owner.block_agent(organization, own.block).await.unwrap();
    let granted = owner.grant(outside.block, account, Access::View).await;
    assert!(granted.is_ok(), "{granted:?}");

    let mut agent = harness.second_connection(&token, organization).await;
    let followed = agent
        .call(|peer| {
            peer.ask::<be_block_protocol::KeyPage>(
                Query::Keys {
                    blocks: None,
                    after: None,
                },
                true,
            )
        })
        .await;
    let Ok(page) = followed else {
        panic!("a block's agent could not follow the keys topic: {followed:?}");
    };
    assert!(
        page.keys.keys.iter().all(|row| row.block == own.block),
        "a block's agent was served another block's keys: {page:?}"
    );

    let seal = |scope: Uuid, sealed: &[u8]| KeySet {
        keys: Vec::new(),
        wrapped: vec![wrap(scope, WrappedBy::Account(account), sealed)],
    };
    let opened = |scoped: &Scoped, sealed: &[u8]| {
        let data = Uuid::new_v4();
        KeySet {
            keys: vec![row(data, scoped.block, KeyKind::Data)],
            wrapped: vec![wrap(data, WrappedBy::Key(scoped.scope), sealed)],
        }
    };
    for (keys, told, place) in [
        (seal(outside.scope, b"outside"), false, "sealed to it outside its block"),
        (opened(&outside, b"outside"), false, "wrapped under a scope outside its block"),
        (opened(&own, b"inside"), true, "wrapped under its block's scope"),
        (seal(own.scope, b"inside"), true, "sealed to it in its block"),
    ] {
        let put = owner
            .call(|peer| {
                peer.write::<()>(be_block_protocol::Op::PutKeys(
                    be_block_protocol::PutKeys::Rows(keys),
                ))
            })
            .await;
        assert!(put.is_ok(), "{put:?}");
        assert_eq!(
            agent.told(Topic::Keys).await,
            told,
            "whether a block's agent heard of a key {place}"
        );
    }

    harness.stop().await;
}
