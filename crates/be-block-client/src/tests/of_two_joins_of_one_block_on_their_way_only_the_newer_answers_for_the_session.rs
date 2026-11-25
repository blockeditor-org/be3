use be_block_session::{OpId, Submission};
use be_type_example_counter::CounterContent;

use super::*;
use crate::{Event, peer};

/// Of two joins of one block on their way at once, the server answers the earlier first, and
/// the newer one's answer is the session as it is: the earlier one, answered to its caller
/// after the newer, tells subscribers nothing (not who hosted as of it) and lets go of nothing
/// the newer one holds.
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

    let connection = laptop.connection();
    let earlier = connection.join_session(laptop.organization(), block, BUILD);
    let newer = connection.join_session(laptop.organization(), block, BUILD);
    let (client, ..) = newer.await.unwrap();
    let (stale, ..) = earlier.await.unwrap();
    assert_eq!(stale, client, "one connection is one client in the session");

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
    assert_eq!(joined, 1, "only the newer join tells who hosts as of it");
}
