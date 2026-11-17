use super::*;

use be_block_graph::Access;
use be_type_text::{TextBlock, TextContent};

/// A rotation leaves a trashed block out, so its key stays wrapped under the scope's old key
/// only, which the server's check of a move does not take as the new one: restoring it wraps it
/// under the scope's key now (`Peer::rescope`), and another device reads it there.
#[tokio::test]
async fn a_block_restored_into_a_scope_rotated_while_it_was_in_the_trash_is_wrapped_under_its_new_key()
 {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let leaver = harness.member(&owner, "leaver@example.com").await;
    let team = workspace(&owner, "Team").await;
    let public = owner
        .verified_identity(leaver.account())
        .expect("the invitation made them contacts");
    owner
        .share(team, leaver.account(), Access::View, &public)
        .await
        .unwrap();
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    owner
        .save(note, &TextBlock::of("kept in the trash"), None)
        .await
        .unwrap();
    owner
        .set_parent(note, BlockParent::Detached)
        .await
        .unwrap();

    owner
        .grant(team, leaver.account(), Access::None)
        .await
        .unwrap();
    owner.tend().await.unwrap();
    owner
        .set_parent(note, BlockParent::Block(team))
        .await
        .expect("restoring into the rotated scope left the block under its old key");

    let laptop = harness.second(&owner).await;
    assert_eq!(text(&laptop, note).await, "kept in the trash");

    harness.stop().await;
}
