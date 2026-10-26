use super::*;
use be_graph::Access;

async fn listed(client: &mut TestClient) -> Vec<(Uuid, Access)> {
    let mut found = Vec::new();
    let mut parents = vec![BlockParent::Root];
    while let Some(parent) = parents.pop() {
        let response = client
            .send(|request| ClientMessage::ListChildren { request, parent })
            .await;
        let ServerMessage::Blocks { blocks, .. } = response else {
            panic!("listing failed: {response:?}");
        };
        for block in blocks {
            parents.push(BlockParent::Block(block.id));
            found.push((block.id, block.access));
        }
    }
    found
}

#[tokio::test]
async fn an_editor_sees_only_the_blocks_it_made_or_was_given() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("shared").await;
    let folder = owner.create_block(BlockParent::Root).await;
    let inside = owner.create_block(BlockParent::Block(folder)).await;
    let private = owner.create_block(BlockParent::Root).await;

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
    assert!(listed(&mut guest).await.is_empty());

    let own = guest.create_block(BlockParent::Root).await;
    assert_eq!(listed(&mut guest).await, [(own, Access::Manage)]);

    let account = guest_account(&mut guest).await;
    let response = owner
        .send(|request| ClientMessage::SetAccess {
            request,
            block: folder,
            account,
            access: Access::View,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let mut seen = listed(&mut guest).await;
    seen.sort();
    let mut expected = vec![
        (own, Access::Manage),
        (folder, Access::View),
        (inside, Access::View),
    ];
    expected.sort();
    assert_eq!(seen, expected);
    assert!(!seen.iter().any(|(id, _)| *id == private));

    let owned = listed(&mut owner).await;
    assert_eq!(owned.len(), 4);
    assert!(owned.iter().all(|(_, access)| *access == Access::Manage));

    harness.stop().await;
}

async fn guest_account(guest: &mut TestClient) -> Uuid {
    let token = guest.token.clone();
    let response = guest
        .send(|request| ClientMessage::Authenticate { request, token })
        .await;
    let ServerMessage::Authenticated { account, .. } = response else {
        panic!("the guest could not re-authenticate: {response:?}");
    };
    account
}
