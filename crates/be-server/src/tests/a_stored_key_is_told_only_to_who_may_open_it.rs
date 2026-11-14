use super::*;
use be_block_graph::Access;
use be_wasm_ops::net::Stream;

/// A member of `organization` that accepted the owner's invitation, so a contact of it, with
/// `keys-changed` open.
async fn member(
    harness: &Harness,
    owner: &mut TestClient,
    organization: Uuid,
    email: &str,
) -> (TestClient, Uuid, Stream<()>) {
    let mut member = harness.client().await;
    let account = member.register(email).await;
    let link = owner
        .invite(organization, email)
        .await
        .expect("inviting failed");
    let accepted = member.accept(link, &[]).await;
    assert!(accepted.is_ok(), "{accepted:?}");
    member.open(organization).await;
    let told = client::keys::keys_changed(&member.peer);
    (member, account, told)
}

/// Whether `told` holds an item, after two round trips on its connection: a push queued for it
/// before the first is sent before the second's ping arrives, whichever the server took first.
async fn told(client: &mut TestClient, told: &mut Stream<()>) -> bool {
    client.round_trip().await;
    client.round_trip().await;
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    told.poll_recv(&mut context).is_ready()
}

#[tokio::test]
async fn a_stored_key_is_told_only_to_who_may_open_it() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let organization = owner.organization("keys").await;
    let folder = owner.scope_root(owner_id, BlockParent::Root).await;
    let mut owners = client::keys::keys_changed(&owner.peer);
    let (mut grantee, grantee_id, mut grantees) =
        member(&harness, &mut owner, organization, "grantee@example.com").await;
    let (mut sealed, sealed_id, mut sealeds) =
        member(&harness, &mut owner, organization, "sealed@example.com").await;
    let (mut bystander, _, mut bystanders) =
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
        .call(|peer| client::keys::put_keys(peer, seals.clone()))
        .await;
    assert!(put.is_ok(), "{put:?}");
    assert_eq!(
        grantees.recv().await,
        Some(()),
        "the grantee was not told of the key sealed to it"
    );
    assert!(
        !told(&mut sealed, &mut sealeds).await,
        "a contact sealed a key to that may not view its block was told"
    );
    assert!(
        !told(&mut bystander, &mut bystanders).await,
        "a member nothing was shared with was told"
    );
    assert!(
        !told(&mut owner, &mut owners).await,
        "the connection that stored the keys was told"
    );

    let again = owner.call(|peer| client::keys::put_keys(peer, seals)).await;
    assert!(again.is_ok(), "{again:?}");
    assert!(
        !told(&mut grantee, &mut grantees).await,
        "keys put again that stored nothing were told"
    );

    harness.stop().await;
}
