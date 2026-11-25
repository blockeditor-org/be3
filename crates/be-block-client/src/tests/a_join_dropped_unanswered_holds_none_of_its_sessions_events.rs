use std::time::Duration;

use be_block_session::{OpId, Submission};
use be_type_example_counter::CounterContent;

use super::*;
use crate::{Event, peer};

/// A join given up before its answer (a caller that stopped waiting) stops holding its block's
/// session events: what the session tells afterwards reaches subscribers, rather than waiting
/// for an answer nothing will take.
#[tokio::test]
async fn a_join_dropped_unanswered_holds_none_of_its_sessions_events() {
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
    laptop
        .connection()
        .join_session(laptop.organization(), block, BUILD)
        .await
        .unwrap();

    drop(
        laptop
            .connection()
            .join_session(laptop.organization(), block, BUILD),
    );
    // Answered after the server took the join it was asked first: the laptop is in the session.
    laptop
        .connection()
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
    let Ok(peer::Submitted::Server(answer)) = phone
        .submit(block, BUILD, submission, true, Vec::new())
        .await
    else {
        panic!("an online submission goes to the server");
    };
    answer.await.unwrap();
    let heard = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if let Event::Sequenced { block: of, entry } = on_laptop.recv().await.unwrap()
                && of == block
            {
                break entry;
            }
        }
    })
    .await
    .expect("the laptop hears the entry, held by no join");
    assert_eq!(heard.sequence, 1);
}
