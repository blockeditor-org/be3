use super::*;
use be_graph::Access;

#[tokio::test]
async fn a_move_never_takes_a_workspace_or_more_access_without_manage() {
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
    let document = owner.create_block(BlockParent::Block(workspace)).await;
    let folder = owner.create_block(BlockParent::Block(workspace)).await;
    let granted = owner.grant(workspace, editor_id, Access::Edit).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");
    let elsewhere = editor.create_block(BlockParent::Root).await;
    let own = editor.create_block(BlockParent::Block(workspace)).await;
    assert_eq!(editor.read_block(own).await.access, Access::Manage);

    for (block, parent) in [
        (workspace, BlockParent::Block(elsewhere)),
        (workspace, BlockParent::Detached),
        (document, BlockParent::Block(elsewhere)),
        (document, BlockParent::Root),
        (document, BlockParent::Block(own)),
    ] {
        let moved = editor.move_to(block, parent).await;
        assert!(
            refused_as(&moved, ErrorCode::PermissionDenied),
            "an editor moved {block} to {parent:?}: {moved:?}"
        );
    }
    assert_eq!(
        owner.read_block(workspace).await.parent,
        Some(BlockParent::Root)
    );
    assert_eq!(owner.read_block(workspace).await.access, Access::Manage);
    assert_eq!(editor.read_block(document).await.access, Access::Edit);

    let moved = editor.move_to(document, BlockParent::Block(folder)).await;
    assert!(matches!(moved, ServerMessage::Ok { .. }), "{moved:?}");
    let moved = editor.move_to(own, BlockParent::Block(elsewhere)).await;
    assert!(matches!(moved, ServerMessage::Ok { .. }), "{moved:?}");
    let moved = owner.move_to(document, BlockParent::Root).await;
    assert!(matches!(moved, ServerMessage::Ok { .. }), "{moved:?}");

    harness.stop().await;
}
