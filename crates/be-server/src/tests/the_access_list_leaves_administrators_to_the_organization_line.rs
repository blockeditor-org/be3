use super::*;
use be_graph::Access;
use be_protocol::AccessList;

async fn join(
    owner: &mut TestClient,
    member: &mut TestClient,
    organization: Uuid,
    invitation: u128,
    email: &str,
    role: OrganizationRole,
) {
    let response = owner
        .send(|request| ClientMessage::Invite {
            request,
            invitation: Uuid::from_u128(invitation),
            organization,
            email: email.into(),
            role,
            sealed: Vec::new(),
            code: Vec::new(),
            escrow: Vec::new(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    member
        .send(|request| ClientMessage::AcceptInvitation {
            request,
            invitation: Uuid::from_u128(invitation),
            card: Vec::new(),
            contact: Vec::new(),
            escrow: Vec::new(),
        })
        .await;
    member.open(organization).await;
}

async fn access_list(client: &mut TestClient, block: Uuid) -> AccessList {
    let response = client
        .send(|request| ClientMessage::ListAccess { request, block })
        .await;
    let ServerMessage::AccessList { list, .. } = response else {
        panic!("listing access failed: {response:?}");
    };
    list
}

#[tokio::test]
async fn the_access_list_leaves_administrators_to_the_organization_line() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let organization = owner.organization("Company").await;
    let mut boss = harness.member("boss@example.com").await;
    join(
        &mut owner,
        &mut boss,
        organization,
        1,
        "boss@example.com",
        OrganizationRole::Administrator,
    )
    .await;
    let mut guest = harness.client().await;
    let guest_id = guest.register("guest@example.com").await;
    join(
        &mut owner,
        &mut guest,
        organization,
        2,
        "guest@example.com",
        OrganizationRole::Editor,
    )
    .await;
    let folder = owner.create_block(BlockParent::Root).await;
    let inner = owner.create_block(BlockParent::Block(folder)).await;
    let granted = owner.grant(folder, guest_id, Access::Edit).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");

    let list = access_list(&mut owner, inner).await;
    assert!(list.company);
    let accounts: Vec<Uuid> = list.entries.iter().map(|entry| entry.account).collect();
    assert_eq!(
        accounts
            .iter()
            .filter(|account| **account == owner_id)
            .count(),
        1,
        "the one listing is listed"
    );
    assert!(
        accounts.contains(&guest_id),
        "someone who inherits access is listed"
    );
    assert_eq!(
        accounts.len(),
        2,
        "the other administrator is left to the organization's line: {accounts:?}"
    );
    let entry = |list: &AccessList| {
        list.entries
            .iter()
            .find(|entry| entry.account == guest_id)
            .cloned()
            .unwrap()
    };
    let guest_entry = entry(&list);
    assert_eq!(guest_entry.granted, None);
    assert_eq!(guest_entry.inherited, Access::Edit);
    assert_eq!(guest_entry.effective, Access::Edit);

    let granted = owner.grant(inner, guest_id, Access::Manage).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    let guest_entry = entry(&access_list(&mut owner, inner).await);
    assert_eq!(guest_entry.granted, Some(Access::Manage));
    assert_eq!(
        guest_entry.inherited,
        Access::Edit,
        "what comes from above is told apart from the grant here"
    );
    assert_eq!(guest_entry.effective, Access::Manage);

    let seen_by_boss = access_list(&mut boss, inner).await;
    assert!(
        !seen_by_boss
            .entries
            .iter()
            .any(|entry| entry.account == owner_id),
        "an administrator who made the block is still one of the administrators"
    );
    harness.stop().await;
}
