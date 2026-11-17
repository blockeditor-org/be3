use super::*;
use be_block_graph::Access;

/// A member of `organization` that accepted the owner's invitation, so a contact of it, with
/// the keys topic followed.
async fn member(
    harness: &Harness,
    owner: &mut TestClient,
    organization: Uuid,
    email: &str,
) -> (TestClient, Uuid) {
    let mut member = harness.client().await;
    let account = member.register(email).await;
    let link = owner.invite(organization).await.expect("inviting failed");
    let accepted = member.accept(link, &[]).await;
    assert!(accepted.is_ok(), "{accepted:?}");
    member.open(organization).await;
    follow_keys(&mut member).await;
    (member, account)
}

async fn follow_keys(client: &mut TestClient) {
    let followed = client
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
    assert!(followed.is_ok(), "{followed:?}");
}

#[tokio::test]
async fn a_stored_key_is_told_only_to_who_may_open_it() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let organization = owner.organization("keys").await;
    let folder = owner.scope_root(owner_id, BlockParent::Root).await;
    follow_keys(&mut owner).await;
    let (mut grantee, grantee_id) =
        member(&harness, &mut owner, organization, "grantee@example.com").await;
    let (mut sealed, sealed_id) =
        member(&harness, &mut owner, organization, "sealed@example.com").await;
    let (mut bystander, _) =
        member(&harness, &mut owner, organization, "bystander@example.com").await;
    let granted = owner.grant(folder.block, grantee_id, Access::View).await;
    assert!(granted.is_ok(), "{granted:?}");

    let seals = KeySet {
        keys: Vec::new(),
        wrapped: vec![
            wrap(
                folder.scope,
                WrappedBy::Account(grantee_id),
                b"for the grantee",
            ),
            wrap(
                folder.scope,
                WrappedBy::Account(sealed_id),
                b"for a contact",
            ),
        ],
    };
    let put = owner
        .call(|peer| peer.write::<()>(be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Rows(seals.clone()))))
        .await;
    assert!(put.is_ok(), "{put:?}");
    grantee.heard_changed(Topic::Keys).await;
    assert!(
        !sealed.told(Topic::Keys).await,
        "a contact sealed a key to that may not view its block was told"
    );
    assert!(
        !bystander.told(Topic::Keys).await,
        "a member nothing was shared with was told"
    );
    assert!(
        !owner.told(Topic::Keys).await,
        "the connection that stored the keys was told"
    );

    let again = owner.call(|peer| peer.write::<()>(be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Rows(seals)))).await;
    assert!(again.is_ok(), "{again:?}");
    assert!(
        !grantee.told(Topic::Keys).await,
        "keys put again that stored nothing were told"
    );

    harness.stop().await;
}
