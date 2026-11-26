use super::*;
use be_block_graph::Access;
use be_block_session::Membership;

/// An editor that holds the lease and is made a viewer loses it at once: the server writes the
/// change of access into the session's order, and the next editor holds the lease from it.
#[tokio::test]
async fn a_holder_that_loses_edit_access_hands_the_lease_on() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.member("guest@example.com").await;
    let guest_account = guest.id();
    guest.personal().await;
    let link = owner.link().await;
    let _ = guest.accept(link, &[]).await;
    let block = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(block, guest_account, Access::Edit).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");

    let (holder, state) = guest.join_session(block).await;
    assert!(
        state.is_holder(holder),
        "an editor joining first holds the lease"
    );
    let (author, mut seen) = owner.join_session(block).await;
    assert!(seen.is_holder(holder));

    let demoted = owner.grant(block, guest_account, Access::View).await;
    assert!(matches!(demoted, Ok(())), "{demoted:?}");
    let index = seen.index + 1;
    owner.membership_until(block, &mut seen, index).await;
    assert!(seen.is_holder(author), "the lease went to the editor");
    assert!(seen.has(holder), "the viewer stays in the session");
    harness.stop().await;
}
