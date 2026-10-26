use super::*;
use be_graph::{Access, SETTINGS_TYPE};

async fn set_references(
    client: &mut TestClient,
    block: Uuid,
    references: Vec<Uuid>,
) -> ServerMessage {
    client
        .send(|request| ClientMessage::SetMetadata {
            request,
            block,
            metadata: b"named".to_vec(),
            references,
        })
        .await
}

#[tokio::test]
async fn only_a_manager_lets_go_of_a_blocks_settings() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut editor = harness.client().await;
    let editor_id = editor.register("editor@example.com").await;
    editor.personal().await;
    let link = owner.link(None).await;
    editor.accept(link, &[]).await;

    let workspace = owner.create_block(BlockParent::Root).await;
    let settings = Uuid::new_v4();
    let key = Uuid::new_v4();
    let created = owner
        .send(|request| ClientMessage::CreateBlock {
            request,
            block: settings,
            content_type: SETTINGS_TYPE,
            parent: BlockParent::Block(workspace),
            metadata: Vec::new(),
            references: Vec::new(),
            key,
            keys: KeySet {
                keys: vec![row(key, settings, KeyKind::Data)],
                wrapped: Vec::new(),
            },
        })
        .await;
    assert!(
        matches!(created, ServerMessage::Block { .. }),
        "{created:?}"
    );
    let held = set_references(&mut owner, workspace, vec![settings]).await;
    assert!(matches!(held, ServerMessage::Block { .. }), "{held:?}");

    let granted = owner.grant(workspace, editor_id, Access::Edit).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    let kept = set_references(&mut editor, workspace, vec![settings]).await;
    assert!(matches!(kept, ServerMessage::Block { .. }), "{kept:?}");
    let dropped = set_references(&mut editor, workspace, Vec::new()).await;
    assert!(
        refused_as(&dropped, ErrorCode::PermissionDenied),
        "an editor let go of the workspace's settings: {dropped:?}"
    );
    let dropped = set_references(&mut owner, workspace, Vec::new()).await;
    assert!(
        matches!(dropped, ServerMessage::Block { .. }),
        "{dropped:?}"
    );

    harness.stop().await;
}
