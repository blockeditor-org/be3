use super::*;
use be_block_graph::Access;

/// A's block "1", shared with B to edit, and B's block "2" inside it, published by B.
pub(super) struct SharedTree {
    pub(super) owner: TestClient,
    pub(super) editor: TestClient,
    pub(super) child: Uuid,
    pub(super) size: u64,
}

pub(super) async fn shared_tree(harness: &Harness) -> SharedTree {
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut editor = harness.client().await;
    let editor_id = editor.register("editor@example.com").await;
    editor.personal().await;
    let link = owner.link(None).await;
    let accepted = editor.accept(link, &[]).await;
    assert!(accepted.is_ok(), "{accepted:?}");
    let shared = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(shared, editor_id, Access::Edit).await;
    assert!(granted.is_ok(), "{granted:?}");

    let child = editor.create_block(BlockParent::Block(shared)).await;
    let author = Author::new(editor_id);
    let (head, chunks) = author.compose(b"what the editor wrote inside\n", 1_000, None);
    let published = editor
        .publish(&author, child, head, chunks.clone(), None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    let size = author
        .objects_of(head, &chunks)
        .iter()
        .map(|hash| author.store.get(*hash).unwrap().unwrap().len() as u64)
        .sum();
    SharedTree {
        owner,
        editor,
        child,
        size,
    }
}

pub(super) async fn held(client: &mut TestClient) -> u64 {
    client.storage_usage().await.unwrap().held
}

#[tokio::test]
async fn the_trees_owner_pays_for_what_anyone_makes_in_it() {
    let harness = Harness::start().await;
    let SharedTree {
        mut owner,
        mut editor,
        child,
        size,
    } = shared_tree(&harness).await;
    assert_eq!(
        held(&mut owner).await,
        size,
        "a block made inside someone's tree is charged to that tree's owner"
    );
    assert_eq!(held(&mut editor).await, 0, "its author is not charged");

    let moved = owner.move_to(child, BlockParent::Root).await;
    assert!(moved.is_ok(), "{moved:?}");
    assert_eq!(
        (held(&mut owner).await, held(&mut editor).await),
        (size, 0),
        "a block moved to the top is charged to whoever moved it, never to its author"
    );

    let own = editor.create_block(BlockParent::Root).await;
    let moved = editor.move_to(child, BlockParent::Block(own)).await;
    assert!(moved.is_ok(), "{moved:?}");
    assert_eq!(
        (held(&mut owner).await, held(&mut editor).await),
        (0, size),
        "a block moved into someone's tree takes its bytes to that tree's owner"
    );

    harness.stop().await;
}
