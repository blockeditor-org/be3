use super::*;
use be_graph::{Access, SETTINGS_TYPE};

async fn set_references(
    client: &mut TestClient,
    block: Uuid,
    references: Vec<Uuid>,
) -> Result<BlockSummary, Error> {
    client
        .call(|peer| client::blocks::set_metadata(peer, block, b"named".to_vec(), references))
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
    let _ = editor.accept(link, &[]).await;

    let workspace = owner.create_block(BlockParent::Root).await;
    let settings = Uuid::new_v4();
    let key = Uuid::new_v4();
    let created = owner
        .call(|peer| {
            client::blocks::create_block(
                peer,
                settings,
                SETTINGS_TYPE,
                BlockParent::Block(workspace),
                Vec::new(),
                Vec::new(),
                key,
                KeySet {
                    keys: vec![row(key, settings, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
            )
        })
        .await;
    assert!(matches!(created, Ok(_)), "{created:?}");
    let held = set_references(&mut owner, workspace, vec![settings]).await;
    assert!(matches!(held, Ok(_)), "{held:?}");

    let granted = owner.grant(workspace, editor_id, Access::Edit).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    let kept = set_references(&mut editor, workspace, vec![settings]).await;
    assert!(matches!(kept, Ok(_)), "{kept:?}");
    let dropped = set_references(&mut editor, workspace, Vec::new()).await;
    assert!(
        refused_as(&dropped, ErrorCode::PermissionDenied),
        "an editor let go of the workspace's settings: {dropped:?}"
    );
    let dropped = set_references(&mut owner, workspace, Vec::new()).await;
    assert!(matches!(dropped, Ok(_)), "{dropped:?}");

    harness.stop().await;
}
