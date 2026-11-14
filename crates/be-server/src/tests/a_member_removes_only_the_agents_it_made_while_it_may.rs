use super::*;
use be_block_graph::Access;

/// Anyone but an administrator removes only an agent it made, and a block's agent only while it
/// still manages the block.
#[tokio::test]
async fn a_member_removes_only_the_agents_it_made_while_it_may() {
    let harness = Harness::start().await;
    let mut admin = harness.client().await;
    admin.register("admin@example.com").await;
    let organization = admin.organization("team").await;
    let mut creator = harness.client().await;
    let creator_id = creator.register("creator@example.com").await;
    let mut colleague = harness.client().await;
    let colleague_id = colleague.register("colleague@example.com").await;
    for (member, email) in [
        (&mut creator, "creator@example.com"),
        (&mut colleague, "colleague@example.com"),
    ] {
        let invitation = admin
            .invite(organization, email)
            .await
            .expect("an administrator invites");
        let joined = member.accept(invitation, &[]).await;
        assert!(matches!(joined, Ok(())), "{joined:?}");
        member.open(organization).await;
    }

    let refused = creator
        .call(|peer| client::members::remove_member(peer, colleague_id))
        .await;
    assert!(
        refused_as(&refused, ErrorCode::MemberNotFound),
        "an editor removed a person: {refused:?}"
    );

    let block = admin.create_block(BlockParent::Root).await;
    let granted = admin.grant(block, creator_id, Access::Manage).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    let (agent, _) = creator
        .block_agent(organization, block)
        .await
        .expect("the creator makes the block's agent");
    // In the trash the agent waits for Restore, so lowering the creator's grant keeps it.
    let trashed = admin.move_to(block, BlockParent::Detached).await;
    assert!(matches!(trashed, Ok(())), "{trashed:?}");
    let lowered = admin.grant(block, creator_id, Access::View).await;
    assert!(matches!(lowered, Ok(())), "{lowered:?}");
    let refused = creator
        .call(|peer| client::members::remove_member(peer, agent))
        .await;
    assert!(
        refused_as(&refused, ErrorCode::PermissionDenied),
        "a creator that no longer manages the block removed its agent: {refused:?}"
    );
    let members = admin
        .call(|peer| client::members::list_members(peer))
        .await
        .expect("listing members failed");
    assert!(
        members.iter().any(|member| member.account == agent),
        "{members:?}"
    );

    harness.stop().await;
}
