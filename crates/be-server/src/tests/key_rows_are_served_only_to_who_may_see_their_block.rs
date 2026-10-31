use super::*;
use be_graph::Access;

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
    let response = owner
        .call(|peer| client::blocks::set_access(peer, block, account, access))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
}

#[tokio::test]
async fn key_rows_are_served_only_to_who_may_see_their_block() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let organization = owner.organization("keys").await;
    let folder = owner.scope_root(owner_id, BlockParent::Root).await;
    let (inside, inside_key) = owner.inside(&folder).await;
    owner.lists(owner_id, folder.block, vec![inside]).await;
    let private = owner.scope_root(owner_id, BlockParent::Root).await;

    let mut guest = harness.client().await;
    let guest_id = guest.register("guest@example.com").await;
    let response = owner
        .call(|peer| {
            client::organizations::invite(
                peer,
                Uuid::from_u128(1),
                organization,
                "guest@example.com".into(),
                OrganizationRole::Editor,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let _ = guest
        .call(|peer| {
            client::organizations::accept_invitation(
                peer,
                Uuid::from_u128(1),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
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
            client::keys::put_keys(
                peer,
                KeySet {
                    keys: Vec::new(),
                    wrapped: vec![wrap(
                        folder.scope,
                        WrappedBy::Account(guest_id),
                        b"a viewer sealing itself a key",
                    )],
                },
            )
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
            client::keys::put_keys(
                peer,
                KeySet {
                    keys: Vec::new(),
                    wrapped: vec![wrap(
                        folder.scope,
                        WrappedBy::Account(guest_id),
                        b"scope sealed to the guest",
                    )],
                },
            )
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
        .call(|peer| client::keys::get_recovery_key(peer))
        .await;
    let Ok(be_protocol::RecoveryKey { sealed, .. }) = response else {
        panic!("{response:?}");
    };
    assert_eq!(sealed.len(), 1);

    grant(&mut owner, folder.block, guest_id, Access::None).await;
    assert!(
        guest.keys(None).await.is_empty(),
        "a revoked guest was still served the keys"
    );
    let response = guest
        .call(|peer| client::keys::get_recovery_key(peer))
        .await;
    assert!(
        matches!(&response, Ok(be_protocol::RecoveryKey { sealed, .. }) if sealed.is_empty()),
        "a revoked guest was still served what was sealed to it: {response:?}"
    );

    assert_eq!(
        ids(&owner.keys(Some(vec![private.block])).await),
        sorted(vec![private.data, private.scope])
    );
    let _ = owner
        .call(|peer| client::blocks::delete_block(peer, private.block))
        .await;
    let response = owner
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    assert!(
        matches!(response, Ok(be_protocol::Collected { .. })),
        "{response:?}"
    );
    let left = owner.keys(None).await;
    assert!(
        !ids(&left).contains(&private.data) && !ids(&left).contains(&private.scope),
        "a collected block's keys outlived it"
    );
    assert_eq!(left.keys.len(), 3);

    harness.stop().await;
}
