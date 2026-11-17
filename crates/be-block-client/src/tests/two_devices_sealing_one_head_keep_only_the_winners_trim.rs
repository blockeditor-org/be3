use be_type_text::{TextBlock, TextContent};

use super::*;

/// Two devices that both take themselves for the lease holder seal against one head: the
/// compare-and-swap takes one, and only that seal trims the server's journal, in the same
/// transaction; the other is rejected and trims nothing.
#[tokio::test]
async fn two_devices_sealing_one_head_keep_only_the_winners_trim() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let note = phone
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    let head = phone
        .save(note, &TextBlock::of("first"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let laptop = harness.second(&phone).await;
    // The laptop reads the head both seal on.
    laptop.open::<TextContent>(note).await.unwrap();
    phone.connection().join_session(note, BUILD).await.unwrap();
    laptop.connection().join_session(note, BUILD).await.unwrap();
    for counter in 1..=3 {
        submitted(&phone, note, counter).await;
    }

    let through_one = TextBlock::of("through one");
    let won = phone
        .write(note, sealing(&through_one, head, 1))
        .await
        .unwrap();
    let Saved::Published(won) = won else {
        panic!("the first seal on the head is taken: {won:?}");
    };
    let through_two = TextBlock::of("through two");
    // Sealed and sent as a replica's seal is, without the merge a rejection starts.
    let Sealed::Sealed(late) = laptop
        .seal(note, sealing(&through_two, head, 2))
        .await
        .unwrap()
    else {
        panic!("the laptop's seal changes the content");
    };
    let lost = laptop.publish_sealed(note, late).await.unwrap();
    assert!(
        matches!(lost, Saved::Rejected { head: Some(at) } if at == won),
        "the second seal on the same head is rejected: {lost:?}"
    );

    assert_eq!(
        server_journal(&laptop, note).await,
        Some((Some(won), 1, vec![2, 3])),
        "only the winner's trim is kept"
    );
    harness.stop().await;
}
