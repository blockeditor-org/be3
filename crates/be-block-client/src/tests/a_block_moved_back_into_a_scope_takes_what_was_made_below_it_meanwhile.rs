use super::*;

use be_block_graph::Access;
use be_type_text::{TextBlock, TextContent};

/// A folder moved out of a shared workspace keeps its key wrapped under the workspace's, so
/// moving it back finds that key there already; a note made in it meanwhile was wrapped only
/// where the folder was, and goes back under the workspace's key with it, for its reader.
#[tokio::test]
async fn a_block_moved_back_into_a_scope_takes_what_was_made_below_it_meanwhile() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let reader = harness.member(&owner, "reader@example.com").await;
    let team = workspace(&owner, "Team").await;
    let private = workspace(&owner, "Private").await;
    let public = owner
        .verified_identity(reader.account())
        .expect("the invitation made them contacts");
    owner
        .share(team, reader.account(), Access::View, &public)
        .await
        .unwrap();
    let folder = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();

    owner
        .set_parent(folder, BlockParent::Block(private))
        .await
        .unwrap();
    let note = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    owner
        .save(note, &TextBlock::of("made while away"), None)
        .await
        .unwrap();
    owner
        .set_parent(folder, BlockParent::Block(team))
        .await
        .expect("moving the folder back left the note below it unwrapped");

    reader.refresh_keys(Some(vec![note])).await.unwrap();
    assert_eq!(text(&reader, note).await, "made while away");

    harness.stop().await;
}
