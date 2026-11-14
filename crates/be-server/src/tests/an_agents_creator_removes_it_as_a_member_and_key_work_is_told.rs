use super::*;

#[tokio::test]
async fn an_agents_creator_removes_it_as_a_member_and_key_work_is_told() {
    let harness = Harness::start().await;
    let mut admin = harness.client().await;
    admin.register("admin@example.com").await;
    let organization = admin.organization("team").await;
    let mut creator = harness.client().await;
    let creator_id = creator.register("creator@example.com").await;
    let invitation = admin
        .invite(organization, "creator@example.com")
        .await
        .expect("an administrator invites");
    let joined = creator.accept(invitation, &[]).await;
    assert!(matches!(joined, Ok(())), "{joined:?}");
    creator.open(organization).await;

    let response = creator
        .call(|peer| {
            client::members::create_agent(
                peer,
                organization,
                "Helper".into(),
                [7; 32],
                [7; 32],
                Vec::new(),
                Vec::new(),
                None,
            )
        })
        .await;
    let Ok(be_block_protocol::CreatedAgent { agent, token }) = response else {
        panic!("creating an agent failed: {response:?}");
    };
    let members = admin
        .call(|peer| client::members::list_members(peer))
        .await
        .expect("listing members failed");
    assert!(
        members
            .iter()
            .any(|member| member.account == agent.account && member.created_by == Some(creator_id)),
        "{members:?}"
    );

    let mut work = client::keys::key_work_changed(&admin.peer);
    work.recv().await.expect("the stream starts with the key work");
    let removed = creator
        .call(|peer| client::members::remove_member(peer, agent.account))
        .await;
    assert!(matches!(removed, Ok(())), "{removed:?}");
    work.recv()
        .await
        .expect("removing the agent told the administrator its key work again");

    let members = admin
        .call(|peer| client::members::list_members(peer))
        .await
        .expect("listing members failed");
    assert!(members.iter().all(|member| member.account != agent.account));
    assert!(!TestClient::signs_in(&harness, &token).await);

    harness.stop().await;
}
