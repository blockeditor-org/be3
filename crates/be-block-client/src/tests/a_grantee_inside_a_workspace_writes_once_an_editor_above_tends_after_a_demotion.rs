use super::a_demoted_administrator_with_the_server_reads_nothing_written_after_the_demotion::opened_with_the_server;
use super::*;

use be_block_protocol::OrganizationRole;

/// A grantee who reaches a scope only from inside a workspace cannot give it a new epoch, which
/// is wrapped under the workspace's: after a demotion outdates every epoch under the escrow,
/// their save there is refused (`Locked`; the host holds it and submits it again as keys
/// arrive). The workspace's manager, connected but writing nothing, is told key work changed
/// and rotates the workspace from its top as it tends; the grantee's save then goes through,
/// under an epoch the demoted administrator, with the server, does not open (signed access 8).
#[tokio::test]
async fn a_grantee_inside_a_workspace_writes_once_an_editor_above_tends_after_a_demotion() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let manager = harness.member(&admin, "manager@example.com").await;
    let grantee = harness.member(&admin, "grantee@example.com").await;
    befriend(&manager, &grantee).await;
    let team = workspace(&manager, "Team").await;
    let inner = manager
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let public = manager.verified_identity(grantee.account()).unwrap();
    manager
        .share(inner, grantee.account(), Access::Edit, &public)
        .await
        .unwrap();
    grantee.refresh_keys(None).await.unwrap();
    let head = grantee
        .save(inner, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    assert!(
        grantee.summary(team).await.is_err(),
        "the grantee views the workspace, so the test shows nothing"
    );

    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    let promoted = harness.second(&deputy).await;
    assert_eq!(text(&promoted, inner).await, "before");
    let old = grantee.keys().scope_key(inner).unwrap();
    assert!(opened_with_the_server(&harness, &promoted).contains(&old.to_string()));

    admin
        .set_role(deputy.account(), OrganizationRole::Editor)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    // The grantee's devices read the organization's log as key work tells them of the demotion.
    grantee.tend().await.unwrap();
    let refused = grantee.save(inner, &TextBlock::of("after"), head).await;
    assert!(
        matches!(refused, Err(ClientError::Locked(_))),
        "the grantee wrote under an epoch from before the demotion: {refused:?}"
    );

    let team_before = manager.keys().scope_key(team).unwrap();
    manager.tend().await.unwrap();
    let team_after = manager.keys().scope_key(team).unwrap();
    assert_ne!(team_after, team_before, "tending rotated nothing");
    // Tending again finds nothing more to rotate.
    manager.tend().await.unwrap();
    assert_eq!(manager.keys().scope_key(team), Some(team_after));

    grantee.refresh_keys(None).await.unwrap();
    let saved = grantee
        .save(inner, &TextBlock::of("after"), head)
        .await
        .unwrap()
        .published()
        .unwrap();
    let written = grantee
        .commits()
        .vault()
        .sealed_under(saved.hash())
        .unwrap();
    let opened = opened_with_the_server(&harness, &promoted);
    assert!(
        !opened.contains(&written.to_string()),
        "the demoted administrator, with the server, opens what the grantee wrote"
    );
    let scope = grantee.keys().scope_key(inner).unwrap();
    assert_ne!(
        scope, old,
        "the epoch reached through escrow is still the newest"
    );
    assert!(!opened.contains(&scope.to_string()));
    assert_eq!(text(&manager, inner).await, "after");

    harness.stop().await;
}
