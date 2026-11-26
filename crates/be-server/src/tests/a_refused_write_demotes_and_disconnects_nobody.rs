use super::*;

/// What a write's ops change in memory (a role the hub holds, sessions, connections dropped,
/// pushes) waits for the write to be kept: a write refused at its second op leaves an
/// administrator it would have demoted administering, on the connection it had. A demotion
/// kept reaches that connection's next call without closing it.
#[tokio::test]
async fn a_refused_write_demotes_and_disconnects_nobody() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let mut deputy = harness.member("deputy@example.com").await;
    let joined = owner.invite(organization).await.unwrap();
    assert!(matches!(deputy.accept(joined, &[]).await, Ok(())));
    let account = owner
        .call(|peer| peer.read::<Vec<be_block_protocol::Member>>(Query::Members))
        .await
        .unwrap()
        .into_iter()
        .find(|member| member.email == "deputy@example.com")
        .unwrap()
        .account;
    let promoted = owner
        .set_role(account, OrganizationRole::Administrator)
        .await;
    assert!(matches!(promoted, Ok(())), "{promoted:?}");
    deputy.open(organization).await;
    let administers = |deputy: &mut TestClient| {
        let peer = deputy.peer.clone();
        async move {
            peer.read::<Vec<be_block_protocol::InvitationRow>>(Query::Invitations)
                .await
                .expect("the deputy's connection stayed open")
        }
    };
    assert!(administers(&mut deputy).await.is_ok());

    let demote = owner.role_op(account, OrganizationRole::Editor).await;
    let refused = Op::Publish {
        block: Uuid::new_v4(),
        expected: None,
        commit: None,
        references: be_block_protocol::ReferenceChange::between(&[], &Vec::new()),
        metadata: b"a block that is not there".to_vec(),
        journal: None,
    };
    let written = client::server::write(
        &owner.peer,
        Some(organization),
        vec![demote.clone(), refused],
    )
    .await
    .expect("the connection stayed open");
    assert!(
        matches!(&written, Err(WriteError { index: 1, .. })),
        "{written:?}"
    );
    let still = administers(&mut deputy).await;
    assert!(
        still.is_ok(),
        "a demotion in a refused write reached the deputy: {still:?}"
    );

    let demoted = owner.call(|peer| peer.write::<()>(demote)).await;
    assert!(matches!(demoted, Ok(())), "{demoted:?}");
    let after = administers(&mut deputy).await;
    assert!(
        refused_as(&after, ErrorCode::PermissionDenied),
        "a demotion kept did not reach the deputy's connection: {after:?}"
    );
    deputy.round_trip().await;

    harness.stop().await;
}
