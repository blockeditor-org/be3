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

    let invitation = Uuid::from_u128(7);
    let response = owner
        .send(|request| ClientMessage::Invite {
            request,
            invitation,
            workspace,
            email: "guest@example.com".into(),
            role: WorkspaceRole::Editor,
            sealed: b"sealed to the invite code".to_vec(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");

    let mut outsider = harness.member("outsider@example.com").await;
    for refused in [
        outsider
            .send(|request| ClientMessage::GetInvitation {
                request,
                invitation,
            })
            .await,
        outsider
            .send(|request| ClientMessage::AcceptInvitation {
                request,
                invitation,
            })
            .await,
    ] {
        assert!(
            matches!(refused, ServerMessage::Failed { .. }),
            "an account with another email used the invitation: {refused:?}"
        );
    }

    let fetched = guest
        .send(|request| ClientMessage::GetInvitation {
            request,
            invitation,
        })
        .await;
    let ServerMessage::Invitation {
        invitation: found, ..
    } = fetched
    else {
        panic!("reading the invitation failed: {fetched:?}");
    };
    assert_eq!(found.workspace_name, "shared");
    assert_eq!(found.sealed, b"sealed to the invite code");

    let response = guest
        .send(|request| ClientMessage::AcceptInvitation {
            request,
            invitation,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    guest.open(workspace).await;
    let again = guest
        .send(|request| ClientMessage::GetInvitation {
            request,
            invitation,
        })
        .await;
    assert!(
        matches!(again, ServerMessage::Failed { .. }),
        "the invitation outlived its use: {again:?}"
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
