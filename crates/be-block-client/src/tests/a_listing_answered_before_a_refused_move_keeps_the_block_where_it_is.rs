use be_block_graph::Access;
use be_type_text::TextContent;
use futures_util::FutureExt;

use super::*;

/// A listing of a block's folder asked for while its move waits in the outbox, whose answer is
/// taken only after the drain sent the move and the server refused it: the block never left
/// the folder, so the answer, which lists it there, stands. A rename kept behind the move
/// leaves this device's summary where the move put it.
#[tokio::test]
async fn a_listing_answered_before_a_refused_move_keeps_the_block_where_it_is() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let team = workspace(&admin, "Team").await;
    let locked = workspace(&admin, "Locked").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let public = admin.verified_identity(member.account()).unwrap();
    admin
        .share(team, member.account(), Access::Edit, &public)
        .await
        .unwrap();
    admin
        .share(locked, member.account(), Access::View, &public)
        .await
        .unwrap();

    let folder = Listing::Children(BlockParent::Block(team));
    assert!(
        member.list(folder).await.unwrap().iter().any(|summary| summary.id == note),
        "the member sees the note in its folder"
    );
    member
        .keep_move(note, BlockParent::Block(locked))
        .expect("the move is kept");
    member
        .keep_definition(note, |metadata| {
            metadata.insert(&be_block_content::NAME, &"Moved".to_owned());
        })
        .await
        .expect("the rename is kept");

    // Asked for before the drain sends the move, so answered from before it.
    let mut listing = Box::pin(member.listing(folder));
    assert!(
        listing.as_mut().now_or_never().is_none(),
        "the listing waits for the server's answer"
    );
    let drained = member.drain(|_| false).await.unwrap();
    assert!(
        drained.iter().any(|outcome| matches!(
            outcome,
            Drained::Refused { block, .. } if *block == note
        )),
        "the server refused the move into a folder the member may only view: {drained:?}"
    );

    let listed = listing.await.unwrap();
    assert!(
        listed.blocks.iter().any(|summary| summary.id == note),
        "a refused move took the block out of the folder it is still in: {:?}",
        listed.blocks
    );

    harness.stop().await;
}
