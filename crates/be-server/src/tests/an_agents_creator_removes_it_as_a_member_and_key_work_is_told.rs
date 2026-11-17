use super::*;

#[tokio::test]
async fn an_agents_creator_removes_it_as_a_member_and_key_work_is_told() {
    let harness = Harness::start().await;
    let mut admin = harness.member("admin@example.com").await;
    let organization = admin.organization("team").await;
    let mut creator = harness.member("creator@example.com").await;
    let creator_id = creator.id();
    let invitation = admin
        .invite(organization)
        .await
        .expect("an administrator invites");
    let joined = creator.accept(invitation, &[]).await;
    assert!(matches!(joined, Ok(())), "{joined:?}");
    creator.open(organization).await;
    let signed = admin.sign_in(creator_id).await;
    assert!(matches!(signed, Ok(())), "{signed:?}");

    let response = creator.agent(organization, "Helper").await;
    let Ok(be_block_protocol::CreatedAgent { agent, token }) = response else {
        panic!("creating an agent failed: {response:?}");
    };
    let members = admin
        .call(|peer| peer.read::<Vec<be_block_protocol::Member>>(be_block_protocol::Query::Members))
        .await
        .expect("listing members failed");
    assert!(
        members
            .iter()
            .any(|member| member.account == agent.account && member.created_by == Some(creator_id)),
        "{members:?}"
    );

    let work = admin
        .call(|peer| peer.ask::<be_block_protocol::KeyWork>(Query::KeyWork, true))
        .await;
    assert!(work.is_ok(), "following the key work failed: {work:?}");
    let removed = creator.remove_member(agent.account).await;
    assert!(matches!(removed, Ok(())), "{removed:?}");
    // Removing the agent tells the administrator its key work again.
    admin.heard_changed(Topic::KeyWork).await;

    let members = admin
        .call(|peer| peer.read::<Vec<be_block_protocol::Member>>(be_block_protocol::Query::Members))
        .await
        .expect("listing members failed");
    assert!(members.iter().all(|member| member.account != agent.account));
    assert!(!TestClient::signs_in(&harness, &token).await);

    harness.stop().await;
}
