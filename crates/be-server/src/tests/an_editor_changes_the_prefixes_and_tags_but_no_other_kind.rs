use super::*;
use be_graph::{ANYONE_INSIDE, Access, PREFIXES_TYPE, SETTINGS_TYPE, TAGS_TYPE};

async fn create_under(client: &mut TestClient, content_type: Uuid, parent: Uuid) -> ServerMessage {
    let block = Uuid::new_v4();
    let key = Uuid::new_v4();
    client
        .send(|request| ClientMessage::CreateBlock {
            request,
            block,
            content_type,
            parent: BlockParent::Block(parent),
            metadata: Vec::new(),
            references: Vec::new(),
            key,
            keys: KeySet {
                keys: vec![row(key, block, KeyKind::Data)],
                wrapped: Vec::new(),
            },
        })
        .await
}

fn created(response: &ServerMessage) -> Uuid {
    match response {
        ServerMessage::Block { block, .. } => block.id,
        other => panic!("creating failed: {other:?}"),
    }
}

#[tokio::test]
async fn an_editor_changes_the_prefixes_and_tags_but_no_other_kind() {
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
    let settings = created(&create_under(&mut owner, SETTINGS_TYPE, workspace).await);
    let home = created(&create_under(&mut owner, Uuid::from_u128(0x7a65), settings).await);
    let granted = owner.grant(workspace, editor_id, Access::Edit).await;
    assert!(matches!(granted, ServerMessage::Ok { .. }), "{granted:?}");

    let prefixes = created(&create_under(&mut editor, PREFIXES_TYPE, workspace).await);
    let tags = created(&create_under(&mut editor, TAGS_TYPE, workspace).await);
    for kind in [prefixes, tags] {
        assert_eq!(editor.read_block(kind).await.access, Access::Edit);
        assert_eq!(owner.read_block(kind).await.access, Access::Manage);
        let author = Author::new(editor_id);
        let (commit, chunks) = author.compose(b"kind", 1, None);
        let published = editor.publish(&author, kind, commit, chunks, None, 1).await;
        assert!(
            matches!(published, ServerMessage::Published { .. }),
            "an editor could not change a kind it may: {published:?}"
        );
        let opened = editor.grant(kind, ANYONE_INSIDE, Access::View).await;
        assert!(matches!(opened, ServerMessage::Ok { .. }), "{opened:?}");
        let shared = editor
            .grant(kind, Uuid::from_u128(0x5e), Access::View)
            .await;
        assert!(
            refused_as(&shared, ErrorCode::PermissionDenied),
            "an editor shared a kind: {shared:?}"
        );
    }
    let tag = create_under(&mut editor, Uuid::from_u128(0x7a6), tags).await;
    assert!(matches!(tag, ServerMessage::Block { .. }), "{tag:?}");

    assert_eq!(editor.read_block(home).await.access, Access::View);
    let author = Author::new(editor_id);
    let (commit, chunks) = author.compose(b"home", 1, None);
    let published = editor.publish(&author, home, commit, chunks, None, 1).await;
    assert!(
        refused_as(&published, ErrorCode::PermissionDenied),
        "an editor changed a kind only managers may: {published:?}"
    );
    let planted = create_under(&mut editor, PREFIXES_TYPE, settings).await;
    assert!(
        refused_as(&planted, ErrorCode::PermissionDenied),
        "an editor added a kind to the settings block: {planted:?}"
    );

    harness.stop().await;
}
