use super::*;

/// A session that empties and starts again counts its membership entries on from the highest
/// the registry wrote, so an entry of the earlier session that reaches a participant late never
/// follows on from the new one's: the participant would apply it (a leave of its own) to the
/// new membership.
#[tokio::test]
async fn a_session_started_again_counts_its_membership_past_the_one_before() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    phone.register("phone@example.com").await;
    phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;

    let (_, first) = phone.join_session(block).await;
    phone.leave(block).await;
    phone.round_trip().await;
    let (_, again) = phone.join_session(block).await;
    assert!(
        again.index > first.index + 1,
        "the new session's first entry follows on from the old one's leave: {first:?} {again:?}"
    );
    harness.stop().await;
}
