use be_block_commit::{COMMIT_CLOCK_SLACK, HOUR, MAX_COMMIT_AHEAD, now_milliseconds};
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A device that made a commit offline, then had its clock jump two hours ahead and made another
/// by it, past what the server takes: once it connects and measures the server's clock, it stamps
/// the late one again and sends both (`Peer::restamp`), rather than setting them aside or holding
/// them until the server's clock catches up. The one within the limit is sent as it was.
#[tokio::test]
async fn commits_made_offline_by_a_fast_clock_are_sent_once_the_device_connects() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let team = workspace(&laptop, "Team").await;
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let note = offline
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let _ = offline.save(note, &TextBlock::of("one"), None).await;
    let one = offline.local_head(note).unwrap();
    let one_time = offline.commits().get(one).unwrap().time;
    assert!(one_time <= now_milliseconds() + COMMIT_CLOCK_SLACK);
    // Only this device's clock jumps; the server's, and every other test's, keep the true time.
    offline.set_clock_ahead(2 * HOUR);
    let _ = offline.save(note, &TextBlock::of("two"), Some(one)).await;
    let two = offline.local_head(note).unwrap();
    assert_eq!(offline.pending_commits(note), [one, two]);
    let two_time = offline.commits().get(two).unwrap().time;
    assert!(
        two_time > now_milliseconds() + MAX_COMMIT_AHEAD,
        "the commit was not stamped by the fast clock"
    );

    harness.reconnect(&offline, owner.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Resealed { block: note }),
        "{drained:?}"
    );
    let Some(head) = drained.iter().find_map(|outcome| match outcome {
        Drained::Published { block, head } if *block == note => Some(*head),
        _ => None,
    }) else {
        panic!("the commits made offline were not sent: {drained:?}");
    };
    assert_ne!(head, two, "the commit was sent with its far-future time");
    assert_eq!(offline.resealed_as(head), [two]);
    assert_eq!(offline.pending(), refs::PendingCounts::default());
    assert_eq!(offline.local_head(note), Some(head));
    assert_eq!(offline.known_remote_head(note), Some(head));

    let phone = harness.second(&owner).await;
    assert_eq!(text(&phone, note).await, "two");
    let last = phone.load_reached(head).await.unwrap();
    assert!(
        last.time <= now_milliseconds() + COMMIT_CLOCK_SLACK,
        "the commit was not stamped again by the server's clock"
    );
    // The one within the limit was sent as it was, before the late one was refused: it stays
    // put, and the late one is stamped again on it.
    assert_eq!(last.parents, [one], "the late commit left the one it was made on");
    let first = offline.commits().get(one).unwrap();
    assert_eq!(first.time, one_time, "the commit within the limit moved");
    assert!(first.time <= last.time, "the commits lost their order");
    let original = offline.commits().get(two).unwrap();
    assert_eq!(
        last.change_id(head),
        original.change_id(two),
        "stamped again, the commit became another change"
    );

    harness.stop().await;
}
