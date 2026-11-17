use super::*;
use be_block_graph::{Access, SETTINGS_TYPE};

async fn set_references(
    client: &mut TestClient,
    block: Uuid,
    references: Vec<Uuid>,
) -> Result<BlockSummary, Error> {
    client
        .call(|peer| {
            peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::Publish {
                block: block,
                expected: None,
                commit: None,
                references: be_block_protocol::ReferenceChange::between(&[], &references),
                metadata: b"named".to_vec(),
                journal: None,
            })
        })
        .await
}

#[tokio::test]
async fn only_a_manager_lets_go_of_a_blocks_settings() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let mut editor = harness.member("editor@example.com").await;
    let editor_id = editor.id();
    editor.personal().await;
    let link = owner.link(None).await;
    let _ = editor.accept(link, &[]).await;

    let workspace = owner.create_block(BlockParent::Root).await;
    let settings = Uuid::new_v4();
    let key = Uuid::new_v4();
    let root = owner.root(settings);
    let placement = owner
        .placement(settings, BlockParent::Block(workspace))
        .await;
    let created = owner
        .call(|peer| {
            peer.write_all::<be_block_protocol::BlockSummary>(vec![
                be_block_protocol::Op::CreateBlock {
                    block: settings,
                    content_type: SETTINGS_TYPE,
                    parent: BlockParent::Block(workspace),
                    metadata: Vec::new(),
                    references: Vec::new(),
                    key: key,
                    owner_only: false,
                    root: root,
                    placement: placement,
                },
                be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Rows(KeySet {
                    keys: vec![row(key, settings, KeyKind::Data)],
                    wrapped: Vec::new(),
                })),
            ])
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
