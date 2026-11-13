use super::*;
use be_block_graph::{Access, SEARCH_INDEX_TYPE};

async fn create_index(
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
                SEARCH_INDEX_TYPE,
                BlockParent::Block(parent),
                Vec::new(),
                Vec::new(),
                key,
                KeySet {
                    keys: vec![row(key, block, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
                false,
            )
        })
        .await;
    (block, response)
}

fn refused_with<T: std::fmt::Debug>(response: &Result<T, Error>, message: &str) -> bool {
    matches!(response, Err(error) if error.code == ErrorCode::InvalidRequest && error.message.contains(message))
}

async fn refer(
    client: &mut TestClient,
    block: Uuid,
    references: Vec<Uuid>,
) -> Result<BlockSummary, Error> {
    client
        .call(|peer| client::blocks::set_metadata(peer, block, Vec::new(), references))
        .await
}

#[tokio::test]
async fn a_search_index_takes_exactly_the_access_of_the_block_it_indexes() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut accounts = Vec::new();
    for email in [
        "editor@example.com",
        "viewer@example.com",
        "inside@example.com",
    ] {
        let mut client = harness.client().await;
        let account = client.register(email).await;
        client.personal().await;
        let link = owner.link(None).await;
        let _ = client.accept(link, &[]).await;
        accounts.push((client, account));
    }
    let [
        (mut editor, editor_id),
        (mut viewer, viewer_id),
        (mut inside, inside_id),
    ]: [_; 3] = accounts.try_into().ok().unwrap();

    let workspace = owner.create_block(BlockParent::Root).await;
    let note = owner.create_block(BlockParent::Block(workspace)).await;
    for (account, level) in [(editor_id, Access::Edit), (viewer_id, Access::View)] {
        let granted = owner.grant(workspace, account, level).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
    }
    let granted = owner.grant(note, inside_id, Access::Manage).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");

    let (_, created) = create_index(&mut viewer, workspace).await;
    assert!(
        refused_as(&created, ErrorCode::PermissionDenied),
        "a viewer made the workspace's index: {created:?}"
    );
    let (index, created) = create_index(&mut editor, workspace).await;
    assert!(matches!(created, Ok(_)), "{created:?}");
    let referred = refer(&mut owner, workspace, vec![note, index]).await;
    assert!(matches!(referred, Ok(_)), "{referred:?}");

    assert_eq!(owner.read_block(index).await.access, Access::Manage);
    assert_eq!(
        editor.read_block(index).await.access,
        Access::Edit,
        "authoring the index raised its author"
    );
    assert_eq!(viewer.read_block(index).await.access, Access::View);
    let read = inside
        .try_read(index).await;
    assert!(
        read.is_err(),
        "a block shared alone opened the tree's index: {read:?}"
    );

    let granted = owner.grant(index, viewer_id, Access::Manage).await;
    assert!(
        refused_as(&granted, ErrorCode::InvalidRequest),
        "the index took a grant of its own: {granted:?}"
    );
    let granted = owner.grant(index, inside_id, Access::View).await;
    assert!(
        refused_as(&granted, ErrorCode::InvalidRequest),
        "the index took a grant of its own: {granted:?}"
    );
    assert_eq!(viewer.read_block(index).await.access, Access::View);

    let (_, created) = create_index(&mut editor, workspace).await;
    assert!(
        refused_as(&created, ErrorCode::InvalidRequest),
        "a block took a second index: {created:?}"
    );
    let (_, created) = create_index(&mut editor, note).await;
    assert!(
        refused_as(&created, ErrorCode::InvalidRequest),
        "an index was made under a block inside a tree: {created:?}"
    );
    let second = owner.create_block(BlockParent::Root).await;
    let third = owner.create_block(BlockParent::Root).await;
    for block in [second, third] {
        let granted = owner.grant(block, editor_id, Access::Edit).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
    }
    let (loose, created) = create_index(&mut editor, second).await;
    assert!(matches!(created, Ok(_)), "{created:?}");
    for target in [workspace, third, note] {
        let moved = editor
            .call(|peer| {
                client::blocks::set_parent(
                    peer,
                    loose,
                    BlockParent::Block(target),
                    KeySet::default(),
                )
            })
            .await;
        assert!(
            moved.is_err(),
            "an index moved under another block: {moved:?}"
        );
    }
    // The owner may move it (Manage skips the access checks), so which check refuses shows.
    let child = owner.create_block(BlockParent::Block(second)).await;
    for target in [child, third, workspace] {
        let moved = owner
            .call(|peer| {
                client::blocks::set_parent(
                    peer,
                    loose,
                    BlockParent::Block(target),
                    KeySet::default(),
                )
            })
            .await;
        assert!(
            refused_with(&moved, "stays under the block it indexes"),
            "the owner moved an index under another block: {moved:?}"
        );
    }
    let (_, created) = create_index(&mut owner, child).await;
    assert!(
        refused_with(&created, "under a top-level block"),
        "an index was made under a nested block: {created:?}"
    );
    let referred = refer(&mut viewer, workspace, vec![note, index]).await;
    assert!(
        refused_as(&referred, ErrorCode::PermissionDenied),
        "a viewer changed what the workspace refers to: {referred:?}"
    );

    harness.stop().await;
}
