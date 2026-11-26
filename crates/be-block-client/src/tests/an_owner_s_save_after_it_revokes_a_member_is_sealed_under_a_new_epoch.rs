use super::*;

use be_block_graph::Access;
use be_block_protocol::KeyKind;
use be_type_text::{TextBlock, TextContent};

/// An owner revokes a reader and saves at once: no `tend`, and no other grant or seal has it
/// read the log since. Its own revocation outdates the epoch it read the log at, so the save
/// gives the scope a new epoch first and writes under a key the revoked reader never gets.
#[tokio::test]
async fn an_owner_s_save_after_it_revokes_a_member_is_sealed_under_a_new_epoch() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
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
        .grant(team, bob.account(), Access::View)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    assert_eq!(text(&bob, note).await, "before");

    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    let epochs = admin.keys().keys_of(team, KeyKind::Scope).len();
    admin
        .save(note, &TextBlock::of("after"), before)
        .await
        .unwrap();
    assert_eq!(
        admin.keys().keys_of(team, KeyKind::Scope).len(),
        epochs + 1,
        "the save wrote under the epoch the owner's own revocation outdates"
    );
    let written = admin.keys().keys_of(note, KeyKind::Data).last().unwrap().id;
    bob.refresh_keys(None).await.unwrap();
    assert!(!bob.keys().has(written));
    assert_eq!(text(&harness.second(&admin).await, note).await, "after");

    harness.stop().await;
}
