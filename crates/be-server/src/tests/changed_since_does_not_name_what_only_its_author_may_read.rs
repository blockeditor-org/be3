use be_block_graph::{Access, SETTINGS_TYPE};
use be_block_protocol::ChangeCursor;

use super::*;

async fn left(client: &mut TestClient, root: Uuid) -> Vec<Uuid> {
    let mut removed = Vec::new();
    let mut after = ChangeCursor {
        changed: 0,
        block: Uuid::max(),
    };
    for _ in 0..50 {
        let page = client
            .call(|peer| client::blocks::changed_since(peer, root, after.clone(), 100))
            .await
            .expect("changed-since failed");
        removed.extend(page.removed);
        match page.next {
            Some(next) => after = next,
            None => return removed,
        }
    }
    panic!("the pages never ended");
}

#[tokio::test]
async fn changed_since_does_not_name_what_only_its_author_may_read() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut accounts = Vec::new();
    for email in ["editor@example.com", "viewer@example.com"] {
        let mut client = harness.client().await;
        let account = client.register(email).await;
        client.personal().await;
        let link = owner.link(None).await;
        let _ = client.accept(link, &[]).await;
        accounts.push((client, account));
    }
    let [(mut editor, editor_id), (mut viewer, viewer_id)]: [_; 2] =
        accounts.try_into().ok().unwrap();

    let workspace = owner.create_block(BlockParent::Root).await;
    for (account, level) in [(editor_id, Access::Edit), (viewer_id, Access::View)] {
        let granted = owner.grant(workspace, account, level).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
    }

    // The owner keeps a block only they may read in their settings in the workspace, the
    // editor makes a plain one, and each trashes theirs.
    let settings = Uuid::new_v4();
    let settings_key = Uuid::new_v4();
    owner
        .create_typed(
            settings,
            SETTINGS_TYPE,
            BlockParent::Block(workspace),
            settings_key,
            KeySet {
                keys: vec![row(settings_key, settings, KeyKind::Data)],
                wrapped: Vec::new(),
            },
        )
        .await;
    let secret = Uuid::new_v4();
    let key = Uuid::new_v4();
    let made = owner
        .call(|peer| {
            client::blocks::create_block(
                peer,
                secret,
                CONTENT,
                BlockParent::Block(settings),
                Vec::new(),
                Vec::new(),
                key,
                KeySet {
                    keys: vec![row(key, secret, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
                true,
            )
        })
        .await;
    assert!(made.is_ok(), "{made:?}");
    let plain = editor.create_block(BlockParent::Block(workspace)).await;
    let trashed = owner.move_to(secret, BlockParent::Detached).await;
    assert!(trashed.is_ok(), "{trashed:?}");
    let trashed = editor.move_to(plain, BlockParent::Detached).await;
    assert!(trashed.is_ok(), "{trashed:?}");

    let mut names = left(&mut viewer, workspace).await;
    names.sort();
    assert_eq!(
        names,
        vec![plain],
        "a reader is not told of the owner's own block"
    );
    assert!(
        !left(&mut editor, workspace).await.contains(&secret),
        "nor is an editor"
    );
    assert!(
        left(&mut owner, workspace).await.contains(&secret),
        "its author is"
    );

    harness.stop().await;
}
