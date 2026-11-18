use super::*;

const CONNECTIONS: u64 = 16;
const EACH: u64 = 16;

/// Submissions numbered at once are sent in the order they were numbered: each participant hears
/// the order's entries 1, 2, 3 and on, never one before the one numbered just ahead of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_participant_hears_the_order_in_sequence_however_many_submit_at_once() {
    // The first entry's push is slow, as a busy store makes it, so the next one numbered would
    // overtake it were the order not held.
    crate::sessions::hold_first_push(1_000);
    let harness = Harness::start().await;
    let mut first = harness.member("phone@example.com").await;
    let account = first.id();
    let organization = first.organization("notes").await;
    let block = first.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let saved = author.compose(b"saved\n", 1_000, None);
    let _ = first
        .publish(&author, block, saved.0, saved.1, None, 1_000)
        .await;
    first.join_session(block).await;
    let token = first.token.clone();
    let mut clients = vec![first];
    for _ in 1..CONNECTIONS {
        let mut client = harness.second_connection(&token, organization).await;
        client.join_session(block).await;
        clients.push(client);
    }

    let submitting = clients
        .iter_mut()
        .enumerate()
        .map(|(origin, client)| async move {
            for counter in 1..=EACH {
                client
                    .submit(block, origin as u64 + 1, counter, 0, vec![origin as u8])
                    .await
                    .expect("the order takes the submission");
            }
        });
    futures_util::future::join_all(submitting).await;

    for client in &mut clients {
        let mut heard = Vec::new();
        for _ in 0..CONNECTIONS * EACH {
            heard.push(client.sequenced(block).await.sequence);
        }
        assert_eq!(
            heard,
            (1..=CONNECTIONS * EACH).collect::<Vec<_>>(),
            "every participant hears the order's entries in sequence"
        );
    }
}
