use super::*;

#[tokio::test]
async fn an_invited_account_joins_the_workspace_and_a_logout_ends_the_session() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let workspace = owner.workspace("shared").await;
    let mut guest = harness.member("Guest@Example.com").await;

    let refused = guest
        .send(|request| ClientMessage::OpenWorkspace { request, workspace })
        .await;
    assert!(
        matches!(refused, ServerMessage::Failed { .. }),
        "{refused:?}"
    );

    let response = owner
        .send(|request| ClientMessage::Invite {
            request,
            workspace,
            email: "guest@example.com".into(),
            role: WorkspaceRole::Editor,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");

    let listed = guest
        .send(|request| ClientMessage::ListInvitations { request })
        .await;
    let ServerMessage::Invitations { invitations, .. } = listed else {
        panic!("listing invitations failed: {listed:?}");
    };
    assert_eq!(invitations.len(), 1);
    assert_eq!(invitations[0].workspace_name, "shared");

    let response = guest
        .send(|request| ClientMessage::RespondInvitation {
            request,
            invitation: invitations[0].id,
            accept: true,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    guest.open(workspace).await;
    let listed = guest
        .send(|request| ClientMessage::ListInvitations { request })
        .await;
    assert!(
        matches!(&listed, ServerMessage::Invitations { invitations, .. } if invitations.is_empty()),
        "{listed:?}"
    );

    let token = guest.token.clone();
    let response = guest
        .send(|request| ClientMessage::Logout { request })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let mut returning = harness.client().await;
    let response = returning
        .send(|request| ClientMessage::Authenticate { request, token })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { .. }),
        "{response:?}"
    );

    harness.stop().await;
}
