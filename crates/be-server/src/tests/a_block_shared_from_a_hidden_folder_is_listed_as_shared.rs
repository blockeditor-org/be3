use super::*;
use be_graph::Access;

async fn listed(client: &mut TestClient, shared: bool) -> Vec<Uuid> {
    let response = client
        .send(|request| match shared {
            true => ClientMessage::ListShared { request },
            false => ClientMessage::ListChildren {
                request,
                parent: BlockParent::Root,
            },
        })
        .await;
    let ServerMessage::Blocks { blocks, .. } = response else {
        panic!("listing failed: {response:?}");
    };
    let mut ids: Vec<Uuid> = blocks.into_iter().map(|block| block.id).collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn a_block_shared_from_a_hidden_folder_is_listed_as_shared() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("shared").await;
    let folder = owner.create_block(BlockParent::Root).await;
    let inside = owner.create_block(BlockParent::Block(folder)).await;
    owner.create_block(BlockParent::Block(inside)).await;

    let mut guest = harness.member("guest@example.com").await;
    let response = owner
        .send(|request| ClientMessage::Invite {
            request,
            invitation: Uuid::from_u128(1),
            organization,
            email: "guest@example.com".into(),
            role: OrganizationRole::Editor,
            sealed: Vec::new(),
            code: Vec::new(),
            escrow: Vec::new(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    guest
        .send(|request| ClientMessage::AcceptInvitation {
            request,
            invitation: Uuid::from_u128(1),
            card: Vec::new(),
            contact: Vec::new(),
            escrow: Vec::new(),
        })
        .await;
    guest.open(organization).await;
    let token = guest.token.clone();
    let ServerMessage::Authenticated { account, .. } = guest
        .send(|request| ClientMessage::Authenticate { request, token })
        .await
    else {
        panic!("the guest could not re-authenticate");
    };
    let own = guest.create_block(BlockParent::Root).await;
    assert_eq!(listed(&mut guest, false).await, [own]);
    assert!(listed(&mut guest, true).await.is_empty());

    let response = owner
        .send(|request| ClientMessage::SetAccess {
            request,
            block: inside,
            account,
            access: Access::View,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    assert_eq!(
        listed(&mut guest, true).await,
        [inside],
        "a block shared from a folder the guest cannot see is listed as shared, without what is inside it"
    );
    assert_eq!(listed(&mut guest, false).await, [own]);

    let response = owner
        .send(|request| ClientMessage::SetAccess {
            request,
            block: folder,
            account,
            access: Access::View,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    assert!(
        listed(&mut guest, true).await.is_empty(),
        "once its folder is visible, the block is reached through the folder"
    );
    let mut roots = vec![own, folder];
    roots.sort();
    assert_eq!(listed(&mut guest, false).await, roots);
    assert!(
        listed(&mut owner, true).await.is_empty(),
        "an administrator sees every folder, so nothing is shared into view"
    );

    harness.stop().await;
}
