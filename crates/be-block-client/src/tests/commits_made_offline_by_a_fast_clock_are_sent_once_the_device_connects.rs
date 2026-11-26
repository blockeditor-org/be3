use be_block_commit::{
    COMMIT_CLOCK_SLACK, HOUR, MAX_COMMIT_AHEAD, now_milliseconds,
    run_now_on_the_system_clock, start_now_at,
};
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A device whose clock runs two hours fast stamps what it makes offline by it, past what the
/// server takes: once it connects and measures the server's clock, it stamps those commits
/// again and sends them (`Peer::restamp`), rather than setting them aside or holding them until
/// the server's clock catches up.
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
    // The device's clock jumps two hours ahead while it is offline; each test runs in its own
    // process, so only this test's clock moves.
    start_now_at(now_milliseconds() + 2 * HOUR);
    let note = offline
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let _ = offline.save(note, &TextBlock::of("one"), None).await;
    let one = offline.local_head(note).unwrap();
    let _ = offline.save(note, &TextBlock::of("two"), Some(one)).await;
    let two = offline.local_head(note).unwrap();
    assert_eq!(offline.pending_commits(note), [one, two]);
    run_now_on_the_system_clock();
    assert!(
        offline.commits().get(one).unwrap().time > now_milliseconds() + MAX_COMMIT_AHEAD,
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
    let first = offline.commits().get(last.parents[0]).unwrap();
    assert_ne!(last.parents[0], one, "the second commit still names the first's old id");
    assert!(first.time <= last.time, "the commits lost their order");
    let original = offline.commits().get(two).unwrap();
    assert_eq!(
        last.change_id(head),
        original.change_id(two),
        "stamped again, the commit became another change"
    );
    assert_eq!(
        last.time - first.time,
        original.time - offline.commits().get(one).unwrap().time,
        "the commits lost their spacing"
    );

    harness.stop().await;
}
