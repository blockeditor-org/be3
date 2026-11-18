use super::*;

#[tokio::test]
async fn members_manage_only_the_agents_they_created() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let mut other = harness.member("other@example.com").await;
    let personal = owner.personal().await;
    assert_eq!(other.personal().await, personal);
    let company = owner.organization("Team").await;
    let other_account = other.id();
    owner.open(company).await;
    let invitation = owner.invite(company).await.expect("the owner invites");
    let joined = other.accept(invitation, &[]).await;
    assert!(matches!(joined, Ok(())), "{joined:?}");
    // The owner administers the company, and only is a member of the personal organization.
    for (organization, administers) in [(personal, false), (company, true)] {
        owner.open(organization).await;
        other.open(organization).await;
        let response = other.agent(organization, "Helper").await;
        let Ok(be_block_protocol::CreatedAgent { agent, token }) = response else {
            panic!("a member could not create an agent: {response:?}");
        };
        let listed = owner
            .call(|peer| {
                peer.read::<Vec<be_block_protocol::Member>>(be_block_protocol::Query::Members)
            })
            .await
            .expect("listing members failed");
        assert_eq!(
            listed.iter().any(|member| member.account == agent.account
                && member.created_by == Some(other_account)),
            administers,
            "only an administrator lists another's agent: {listed:?}"
        );
        let listed = other
            .call(|peer| {
                peer.read::<Vec<be_block_protocol::Member>>(be_block_protocol::Query::Members)
            })
            .await
            .expect("listing members failed");
        assert!(
            listed.len() == 1 && listed[0].account == agent.account,
            "a member lists the agents it made, and no one else: {listed:?}"
        );
        if !administers {
            let denied = owner.remove_member(agent.account).await;
            assert!(refused_as(&denied, ErrorCode::MemberNotFound), "{denied:?}");
        }
        let removed = other.remove_member(agent.account).await;
        assert!(matches!(removed, Ok(())), "{removed:?}");
        assert!(harness.sign_in(&token).await.is_err());
    }
    harness.stop().await;
}
