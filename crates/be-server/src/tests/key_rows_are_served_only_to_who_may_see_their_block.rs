use super::*;
use be_block_graph::Access;

fn ids(keys: &KeySet) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = keys.keys.iter().map(|row| row.id).collect();
    ids.sort_unstable();
    ids
}

fn wraps(keys: &KeySet) -> Vec<(Uuid, WrappedBy)> {
    let mut wraps: Vec<_> = keys
        .wrapped
        .iter()
        .map(|wrap| (wrap.key, wrap.wrapped_by))
        .collect();
    wraps.sort_unstable();
    wraps
}

fn sorted(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    ids.sort_unstable();
    ids
}

async fn grant(owner: &mut TestClient, block: Uuid, account: Uuid, access: Access) {
    let response = owner.grant(block, account, access).await;
    assert!(matches!(response, Ok(())), "{response:?}");
}

#[tokio::test]
async fn key_rows_are_served_only_to_who_may_see_their_block() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    let organization = owner.organization("keys").await;
    let folder = owner.scope_root(owner_id, BlockParent::Root).await;
    let (inside, inside_key) = owner.inside(&folder).await;
    owner.lists(owner_id, folder.block, vec![inside]).await;
    let private = owner.scope_root(owner_id, BlockParent::Root).await;

    let mut guest = harness.member("guest@example.com").await;
    let guest_id = guest.id();
    let response = owner
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::CreateLink {
                link: Uuid::from_u128(1),
                card: Vec::new(),
                code: Vec::new(),
                kind: be_block_protocol::LinkKind::Invitation(be_block_protocol::Invitation {
                    organization,
                    escrow: Vec::new(),
                }),
            })
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let _ = guest
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::AcceptLink {
                link: Uuid::from_u128(1),
                card: Vec::new(),
                contact: Vec::new(),
                key: Vec::new(),
            })
        })
        .await;
    guest.open(organization).await;
    assert!(guest.keys(None).await.is_empty());

    grant(&mut owner, folder.block, guest_id, Access::View).await;
    let seen = guest.keys(None).await;
    assert_eq!(
        ids(&seen),
        sorted(vec![folder.data, folder.scope, inside_key]),
        "the guest was not given the key rows of what it can see, or was given more"
    );
    assert_eq!(
        wraps(&seen),
        {
            let mut expected = vec![
                (folder.data, WrappedBy::Key(folder.scope)),
                (inside_key, WrappedBy::Key(folder.scope)),
            ];
            expected.sort_unstable();
            expected
        },
        "the guest was handed a key sealed to another account"
    );

    let refused = guest
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::PutKeys(
                be_block_protocol::PutKeys::Rows(KeySet {
                    keys: Vec::new(),
                    wrapped: vec![wrap(
                        folder.scope,
                        WrappedBy::Account(guest_id),
                        b"a viewer sealing itself a key",
                    )],
                }),
            ))
        })
        .await;
    assert!(
        matches!(
            refused,
            Err(Error {
                code: ErrorCode::PermissionDenied,
                ..
            })
        ),
        "{refused:?}"
    );
    let response = owner
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::PutKeys(
                be_block_protocol::PutKeys::Rows(KeySet {
                    keys: Vec::new(),
                    wrapped: vec![wrap(
                        folder.scope,
                        WrappedBy::Account(guest_id),
                        b"scope sealed to the guest",
                    )],
                }),
            ))
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let narrowed = guest.keys(Some(vec![inside])).await;
    assert_eq!(
        ids(&narrowed),
        sorted(vec![folder.data, folder.scope, inside_key]),
        "asking for one block did not bring the keys of the blocks above it"
    );
    assert!(
        narrowed.wrapped.iter().any(|wrap| wrap.key == folder.scope
            && wrap.wrapped_by == WrappedBy::Account(guest_id)
            && wrap.sealed == b"scope sealed to the guest"),
        "{narrowed:?}"
    );
    let response = guest
        .call(|peer| {
            peer.read::<be_block_protocol::RecoveryKey>(be_block_protocol::Query::RecoveryKey)
        })
        .await;
    let Ok(be_block_protocol::RecoveryKey { sealed, .. }) = response else {
        panic!("{response:?}");
    };
    assert_eq!(sealed.len(), 1);

    grant(&mut owner, folder.block, guest_id, Access::None).await;
    assert!(
        guest.keys(None).await.is_empty(),
        "a revoked guest was still served the keys"
    );
    let response = guest
        .call(|peer| {
            peer.read::<be_block_protocol::RecoveryKey>(be_block_protocol::Query::RecoveryKey)
        })
        .await;
    assert!(
        matches!(&response, Ok(be_block_protocol::RecoveryKey { sealed, .. }) if sealed.is_empty()),
        "a revoked guest was still served what was sealed to it: {response:?}"
    );

    assert_eq!(
        ids(&owner.keys(Some(vec![private.block])).await),
        sorted(vec![private.data, private.scope])
    );
    harness.stop().await;
}
