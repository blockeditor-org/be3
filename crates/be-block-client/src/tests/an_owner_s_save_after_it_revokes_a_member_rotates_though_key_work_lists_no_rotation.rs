use super::*;

use be_block_graph::Access;
use be_block_protocol::{KeyKind, KeyWork};
use be_type_text::{TextBlock, TextContent};

/// As an_owner_s_save_after_it_revokes_a_member_is_sealed_under_a_new_epoch, against a server
/// that leaves the rotation the revocation calls for out of key work: the owner judges its
/// epoch by the revocation it signed, not by what key work lists, so its next save still
/// rotates, and the revoked reader gets nothing written after.
#[tokio::test]
async fn an_owner_s_save_after_it_revokes_a_member_rotates_though_key_work_lists_no_rotation() {
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
    let work = admin.key_work().await.unwrap();
    assert!(
        work.rotations.contains(&team),
        "an honest server lists the rotation"
    );
    let epochs = admin.keys().keys_of(team, KeyKind::Scope).len();
    admin
        .tend_with(KeyWork {
            rotations: Vec::new(),
            ..work
        })
        .await
        .unwrap();
    assert_eq!(
        admin.keys().keys_of(team, KeyKind::Scope).len(),
        epochs,
        "key work without the rotation rotated nothing"
    );
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
