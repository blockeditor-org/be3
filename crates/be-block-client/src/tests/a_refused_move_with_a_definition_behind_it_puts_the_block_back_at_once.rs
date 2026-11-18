use be_block_graph::Access;
use be_type_text::TextContent;

use super::*;

/// A move the server refuses while a definition of the block waits behind it: this device puts
/// the block back where the server holds it as soon as the refusal is known, keeping what the
/// definition behind it gives it, not where the refused move put it until a later read.
#[tokio::test]
async fn a_refused_move_with_a_definition_behind_it_puts_the_block_back_at_once() {
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
    member.list(folder).await.unwrap();

    member
        .keep_move(note, BlockParent::Block(locked))
        .expect("the move is kept");
    member
        .keep_definition(note, |metadata| {
            metadata.insert(&be_block_content::NAME, &"Renamed".to_owned());
        })
        .await
        .expect("the rename is kept");
    // The definition still waits behind the move as its refusal lands, and goes after it.
    let drained = member.drain(|_| false).await.unwrap();
    assert!(
        drained.iter().any(|outcome| matches!(
            outcome,
            Drained::Refused { block, .. } if *block == note
        )),
        "the server refused the move into a folder the member may only view: {drained:?}"
    );

    let summary = member
        .known_summary(note)
        .expect("the device knows the note");
    assert_eq!(
        summary.parent,
        Some(BlockParent::Block(team)),
        "the device still shows the note where the refused move put it"
    );
    assert_eq!(
        member
            .metadata_of_summary(&summary)
            .await
            .unwrap()
            .get(&be_block_content::NAME),
        Some("Renamed".to_owned()),
        "the definition behind the refused move still names it"
    );

    harness.stop().await;
}
