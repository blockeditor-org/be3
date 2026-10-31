use super::*;
use be_graph::{Access, SETTINGS_TYPE};

async fn create_settings(
    client: &mut TestClient,
    parent: Uuid,
) -> (Uuid, Result<BlockSummary, Error>) {
    let block = Uuid::new_v4();
    let key = Uuid::new_v4();
    let response = client
        .call(|peer| {
            client::blocks::create_block(
                peer,
                block,
                SETTINGS_TYPE,
                BlockParent::Block(parent),
                Vec::new(),
                Vec::new(),
                key,
                KeySet {
                    keys: vec![row(key, block, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
            )
        })
        .await;
    (block, response)
}

async fn set_metadata(client: &mut TestClient, block: Uuid) -> Result<BlockSummary, Error> {
    client
        .call(|peer| client::blocks::set_metadata(peer, block, b"changed".to_vec(), Vec::new()))
        .await
}

#[tokio::test]
async fn a_settings_block_changes_only_with_manage_on_its_parent() {
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
    let folder = owner.create_block(BlockParent::Block(workspace)).await;
    let (workspace_settings, created) = create_settings(&mut owner, workspace).await;
    assert!(matches!(created, Ok(_)), "{created:?}");
    let (folder_settings, created) = create_settings(&mut owner, folder).await;
    assert!(matches!(created, Ok(_)), "{created:?}");
    assert_eq!(
        owner.read_block(folder_settings).await.access,
        Access::Manage
    );

    let granted = owner.grant(workspace, editor_id, Access::Edit).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    assert_eq!(editor.read_block(folder).await.access, Access::Edit);
    for settings in [workspace_settings, folder_settings] {
        assert_eq!(editor.read_block(settings).await.access, Access::View);
        let changed = set_metadata(&mut editor, settings).await;
        assert!(
            refused_as(&changed, ErrorCode::PermissionDenied),
            "an editor changed a settings block: {changed:?}"
        );
    }
    let author = Author::new(editor_id);
    let (commit, chunks) = author.compose(b"settings", 1, None);
    let published = editor
        .publish(&author, workspace_settings, commit, chunks, None, 1)
        .await;
    assert!(
        refused_as(&published, ErrorCode::PermissionDenied),
        "an editor published to a settings block: {published:?}"
    );
    let (_, created) = create_settings(&mut editor, folder).await;
    assert!(
        refused_as(&created, ErrorCode::PermissionDenied),
        "an editor added settings to a folder: {created:?}"
    );
    let own = editor.create_block(BlockParent::Root).await;
    let (planted, created) = create_settings(&mut editor, own).await;
    assert!(matches!(created, Ok(_)), "{created:?}");
    let moved = editor
        .call(|peer| {
            client::blocks::set_parent(peer, planted, BlockParent::Block(folder), KeySet::default())
        })
        .await;
    assert!(
        refused_as(&moved, ErrorCode::PermissionDenied),
        "an editor moved settings into a folder: {moved:?}"
    );
    let inside = editor.create_block(BlockParent::Block(folder)).await;
    assert_eq!(editor.read_block(inside).await.access, Access::Manage);

    let granted = owner.grant(folder, editor_id, Access::Manage).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    assert_eq!(
        editor.read_block(folder_settings).await.access,
        Access::Manage
    );
    let changed = set_metadata(&mut editor, folder_settings).await;
    assert!(matches!(changed, Ok(_)), "{changed:?}");
    let changed = set_metadata(&mut editor, workspace_settings).await;
    assert!(
        refused_as(&changed, ErrorCode::PermissionDenied),
        "{changed:?}"
    );

    harness.stop().await;
}
