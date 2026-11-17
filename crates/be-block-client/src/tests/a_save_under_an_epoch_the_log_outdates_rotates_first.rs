use super::*;

use be_block_graph::Access;
use be_block_protocol::KeyKind;
use be_type_text::{TextBlock, TextContent};

/// A manager revokes a reader and no device has rotated yet. An editor whose device has read
/// the log since, so holds the revocation, saves: the newest epoch names a head before it, so
/// the save gives the scope a new epoch first and writes under a key the revoked reader never
/// gets.
#[tokio::test]
async fn a_save_under_an_epoch_the_log_outdates_rotates_first() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let before = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    admin
        .grant(team, carol.account(), Access::Edit)
        .await
        .unwrap();
    admin
        .grant(team, bob.account(), Access::View)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    assert_eq!(text(&carol, note).await, "before");

    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    carol.checked_access(team).await.unwrap();
    let epochs = carol.keys().keys_of(team, KeyKind::Scope).len();
    carol
        .save(note, &TextBlock::of("after"), before)
        .await
        .unwrap();
    assert_eq!(
        carol.keys().keys_of(team, KeyKind::Scope).len(),
        epochs + 1,
        "the save wrote under an epoch the log outdates"
    );
    let written = carol.keys().keys_of(note, KeyKind::Data).last().unwrap().id;
    bob.refresh_keys(None).await.unwrap();
    assert!(!bob.keys().has(written));
    assert_eq!(text(&harness.second(&admin).await, note).await, "after");

    harness.stop().await;
}
