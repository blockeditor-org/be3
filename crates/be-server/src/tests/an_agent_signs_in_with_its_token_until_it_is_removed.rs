use super::*;

#[tokio::test]
async fn an_agent_signs_in_with_its_token_until_it_is_removed() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("tracker").await;

    let response = owner.agent(organization, "Claude").await;
    let Ok(be_block_protocol::CreatedAgent { agent, token }) = response else {
        panic!("creating an agent failed: {response:?}");
    };
    assert_eq!(agent.name, "Claude");

    let mut bot = harness.client().await;
    let response = bot
        .call(|peer| be_block_protocol::client::server::authenticate(peer, Some(token.clone())))
        .await;
    let Ok(Authenticated { account, .. }) = response else {
        panic!("the agent's token did not sign it in: {response:?}");
    };
    assert_eq!(account, agent.account);
    bot.open(organization).await;
    let response = bot.call(|peer| peer.read::<be_block_protocol::RecoveryKey>(be_block_protocol::Query::RecoveryKey)).await;
    let Ok(be_block_protocol::RecoveryKey { public, sealed, .. }) = response else {
        panic!("reading the recovery key failed: {response:?}");
    };
    assert_eq!(public, Some(test_secret(agent.account).public().0));
    assert!(sealed.is_empty());

    let response = owner.call(|peer| peer.read::<Vec<be_block_protocol::Member>>(be_block_protocol::Query::Members)).await;
    let Ok(members) = response else {
        panic!("listing members failed: {response:?}");
    };
    assert!(
        members.iter().any(|member| member.account == agent.account
            && member.created_by.is_some()
            && member.bound_to.is_none()),
        "the agent is a member its creator made: {members:?}"
    );

    let response = owner.remove_member(agent.account).await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let response = owner.remove_member(agent.account).await;
    assert!(
        refused_as(&response, ErrorCode::MemberNotFound),
        "an agent removed already is not found: {response:?}"
    );
    let mut again = harness.client().await;
    let response = again
        .call(|peer| be_block_protocol::client::server::authenticate(peer, Some(token)))
        .await;
    assert!(
        matches!(response, Err(_)),
        "a removed agent's token still works: {response:?}"
    );

    harness.stop().await;
}
