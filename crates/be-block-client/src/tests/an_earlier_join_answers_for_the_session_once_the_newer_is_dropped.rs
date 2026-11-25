use be_block_session::{OpId, Submission};
use be_type_example_counter::CounterContent;

use super::*;
use crate::{Event, peer};

/// Of two joins of one block on their way, the newer dropped before its answer: the earlier
/// one's answer is then the one this connection takes, so it speaks in the session and
/// subscribers hear who hosts as of it, once.
#[tokio::test]
async fn an_earlier_join_answers_for_the_session_once_the_newer_is_dropped() {
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

    let connection = laptop.connection();
    let earlier = connection.join_session(laptop.organization(), block, BUILD);
    drop(connection.join_session(laptop.organization(), block, BUILD));
    earlier.await.unwrap();
    // Answered after the server took both joins: the laptop is in the session.
    connection
        .relay(block, None, b"here".to_vec())
        .await
        .unwrap();

    let submission = Submission {
        id: OpId {
            origin: 1,
            counter: 1,
        },
        base: 0,
        barrier: false,
        payload: b"an edit".to_vec(),
    };
    let Ok(peer::Submitted::Server(answer)) = phone.submit(block, BUILD, submission, true).await
    else {
        panic!("an online submission goes to the server");
    };
    answer.await.unwrap();
    let mut joined = 0;
    loop {
        match on_laptop.recv().await.unwrap() {
            Event::Host {
                block: of,
                joined: true,
                ..
            } if of == block => joined += 1,
            Event::Sequenced { block: of, .. } if of == block => break,
            _ => {}
        }
    }
    assert_eq!(joined, 1, "the earlier join tells who hosts as of it");
}
