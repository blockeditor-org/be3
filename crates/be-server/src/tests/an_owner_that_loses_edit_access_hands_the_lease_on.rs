use super::*;
use be_block_graph::Access;

/// An editor that holds the lease and is made a viewer hands it to an editor at its next
/// heartbeat, rather than keeping a session whose edits it may no longer make.
#[tokio::test]
async fn an_owner_that_loses_edit_access_hands_the_lease_on() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.client().await;
    let guest_account = guest.register("guest@example.com").await;
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;
    let block = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(block, guest_account, Access::Edit).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");

    let (holder, state) = guest.join_session(block).await;
    assert_eq!(
        state.owner,
        Some(holder),
        "an editor joining first takes the lease"
    );
    let (author, _) = owner.join_session(block).await;

    let demoted = owner.grant(block, guest_account, Access::View).await;
    assert!(matches!(demoted, Ok(())), "{demoted:?}");
    let beat = guest.heartbeat(block, state.generation, None).await;
    let Ok(after) = beat else {
        panic!("the heartbeat failed: {beat:?}");
    };
    assert_eq!(after.owner, Some(author), "the lease went to the editor");
    assert!(after.generation > state.generation);
    assert!(
        after.participants.contains(&holder),
        "the viewer stays in the session"
    );
    harness.stop().await;
}
