use be_graph::Access;
use be_protocol::ErrorCode;
use text_block_types::{TextBlock, TextContent};

use super::*;

#[tokio::test]
async fn a_move_refused_to_an_editor_drops_only_the_move() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let member = harness.member(&owner, "member@example.com").await;
    let mine = my_files(&member).await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let base = owner
        .save(note, &TextBlock::of("shared"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let public = owner.keys().contact(member.account()).unwrap();
    owner
        .share(team, member.account(), Access::Edit, &public)
        .await
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&member, &refs, &store).await;
    assert_eq!(text(&laptop, note).await, "shared");
    laptop.summary(mine).await.unwrap();
    drop(laptop);

    let offline = harness.offline(&member, &refs, &store);
    offline
        .set_parent(note, BlockParent::Block(mine))
        .await
        .unwrap();
    assert!(
        offline
            .save(note, &TextBlock::of("moved and written"), Some(base))
            .await
            .is_err()
    );
    let written = offline.local_head(note).unwrap();

    harness.reconnect(&offline, member.token()).await;
    assert_eq!(
        offline.drain(|_| false).await.unwrap(),
        [
            Drained::Refused {
                block: note,
                code: ErrorCode::PermissionDenied
            },
            Drained::Published {
                block: note,
                head: written
            }
        ],
        "a refused move was taken for lost access"
    );
    let counts = offline.pending();
    assert_eq!((counts.operations, counts.unsent), (0, 0));
    assert!(offline.known_summary(note).unwrap().access.can_edit());
    assert_eq!(
        owner.summary(note).await.unwrap().parent,
        Some(BlockParent::Block(team))
    );
    assert_eq!(text(&owner, note).await, "moved and written");

    harness.stop().await;
}
