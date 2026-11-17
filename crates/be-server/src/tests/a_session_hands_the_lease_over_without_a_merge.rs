use super::*;
use be_block_session::Membership;

/// A holder that disconnects leaves its session, which the server writes into the order: the
/// next editor holds the lease from there and continues from the head without a merge.
#[tokio::test]
async fn a_session_hands_the_lease_over_without_a_merge() {
    let harness = Harness::start().await;
    let mut phone = harness.member("phone@example.com").await;
    let account = phone.id();
    let organization = phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);

    let (owner, state) = phone.join_session(block).await;
    assert!(state.is_holder(owner));

    let first = author.compose(b"typed on the phone\n", 1_000, None);
    let _ = phone
        .publish(&author, block, first.0, first.1, None, 1_000)
        .await;

    let token = phone.token.clone();
    let mut laptop = harness.second_connection(&token, organization).await;
    let (on_laptop, mut joined) = laptop.join_session(block).await;
    assert!(joined.is_holder(owner));
    drop(phone);

    let index = joined.index + 1;
    laptop.membership_until(block, &mut joined, index).await;
    assert!(
        joined.is_holder(on_laptop),
        "the session kept a disconnected owner: {joined:?}"
    );
    assert!(!joined.has(owner));

    let second = author.compose(b"continued on the laptop\n", 2_000, Some(first.0));
    let response = laptop
        .publish(&author, block, second.0, second.1, Some(first.0), 2_000)
        .await;
    assert!(
        matches!(
            response,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "the new owner could not continue from the head: {response:?}"
    );

    let mut returned = harness.second_connection(&token, organization).await;
    let remote = returned.read_block(block).await.head;
    assert_eq!(
        resume(&author.commits, Some(first.0), remote).unwrap(),
        Resume::FastForward { to: second.0 },
        "the phone should catch up rather than merge"
    );

    harness.stop().await;
}
