use be_block_session::{OpId, Submission};
use be_type_example_counter::CounterContent;
use tokio::sync::broadcast;

use super::*;
use crate::{Event, peer};

/// Of two joins of one block on their way at once, the server answers the earlier first, and
/// the newer one's answer is the session as it is: however their callers wait for them, only
/// the newer tells subscribers who hosts as of it, and the earlier lets go of nothing the newer
/// holds.
#[tokio::test]
async fn of_two_joins_of_one_block_on_their_way_only_the_newer_answers_for_the_session() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.second(&phone).await;
    let mut on_laptop = laptop.connection().subscribe();
    phone
        .connection()
        .join_session(phone.organization(), block, BUILD)
        .await
        .unwrap();

    for (counter, newer_first) in [(1, true), (2, false)] {
        let connection = laptop.connection();
        let earlier = connection.join_session(laptop.organization(), block, BUILD);
        let newer = connection.join_session(laptop.organization(), block, BUILD);
        let (stale, client) = match newer_first {
            true => {
                let (client, ..) = newer.await.unwrap();
                (earlier.await.unwrap().0, client)
            }
            false => {
                let (stale, ..) = earlier.await.unwrap();
                (stale, newer.await.unwrap().0)
            }
        };
        assert_eq!(stale, client, "one connection is one client in the session");
        let joined = joined_before_an_entry(&phone, block, counter, &mut on_laptop).await;
        assert_eq!(
            joined, 1,
            "only the newer join tells who hosts as of it (newer answered first: {newer_first})"
        );
    }
}

/// Submits an edit on `phone`, and counts the `Event::Host` joins `events` tells before its
/// entry.
async fn joined_before_an_entry(
    phone: &Peer<MemoryStore>,
    block: Uuid,
    counter: u64,
    events: &mut broadcast::Receiver<Event>,
) -> usize {
    let submission = Submission {
        id: OpId { origin: 1, counter },
        base: counter - 1,
        barrier: false,
        payload: b"an edit".to_vec(),
    };
    let Ok(peer::Submitted::Server(answer)) = phone
        .submit(block, BUILD, submission, true, Vec::new())
        .await
    else {
        panic!("an online submission goes to the server");
    };
    answer.await.unwrap();
    let mut joined = 0;
    loop {
        match events.recv().await.unwrap() {
            Event::Host {
                block: of,
                joined: true,
                ..
            } if of == block => joined += 1,
            Event::Sequenced { block: of, .. } if of == block => return joined,
            _ => {}
        }
    }
}
