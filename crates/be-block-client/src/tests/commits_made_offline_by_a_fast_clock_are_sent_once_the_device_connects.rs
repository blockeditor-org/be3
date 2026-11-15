use be_block_commit::{COMMIT_CLOCK_SLACK, MAX_COMMIT_AHEAD, now_milliseconds, retention::HOUR};
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A device whose clock runs two hours fast stamps what it makes offline by it, past what the
/// server takes: once it connects and measures the server's clock, it stamps those commits
/// again and sends them, rather than holding them until the server's clock catches up.
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
    offline.set_clock_ahead(2 * HOUR);
    let note = offline
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let _ = offline.save(note, &TextBlock::of("one"), None).await;
    let one = offline.local_head(note).unwrap();
    let _ = offline.save(note, &TextBlock::of("two"), Some(one)).await;
    let two = offline.local_head(note).unwrap();
    assert_eq!(offline.pending_commits(note), [one, two]);
    assert!(
        offline.load_commit(one).await.unwrap().time > now_milliseconds() + MAX_COMMIT_AHEAD,
        "the commit was not stamped by the fast clock"
    );

    harness.reconnect(&offline, owner.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Created { block: note }),
        "{drained:?}"
    );
    let Some(head) = drained.iter().find_map(|outcome| match outcome {
        Drained::Published { block, head } if *block == note => Some(*head),
        _ => None,
    }) else {
        panic!("the commits made offline were not sent: {drained:?}");
    };
    assert_ne!(head, two, "the commit was sent with its far-future time");
    assert_eq!(offline.pending(), refs::PendingCounts::default());
    assert_eq!(offline.local_head(note), Some(head));
    assert_eq!(offline.known_remote_head(note), Some(head));

    let phone = harness.second(&owner).await;
    assert_eq!(text(&phone, note).await, "two");
    let last = phone.load_commit(head).await.unwrap();
    // The server thins history within a minute, so the first is read where it was made.
    let [first] = last.parents[..] else {
        panic!("the commits made offline were not sent in order");
    };
    assert_ne!(first, one, "the second commit still names the first's old id");
    let first = offline.load_commit(first).await.unwrap();
    assert!(first.parents.is_empty());
    assert!(first.time <= last.time, "the commits lost their order");
    assert!(
        last.time <= now_milliseconds() + COMMIT_CLOCK_SLACK,
        "the commit was not stamped again by the server's clock"
    );
    let original = offline.load_commit(two).await.unwrap();
    assert_eq!(
        last.change_id(head),
        original.change_id(two),
        "stamped again, the commit became another change"
    );

    harness.stop().await;
}
