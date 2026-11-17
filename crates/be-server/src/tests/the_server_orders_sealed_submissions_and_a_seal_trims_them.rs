use be_block_protocol::JournalBase;

use super::*;

/// The server numbers what a session's participants submit, sealed, and sends each entry to
/// every participant, its author too, in one order; a submission it numbered already is answered
/// and not numbered again; and the holder's seal trims the journal to what came after it as its
/// commit wins the head, which every participant hears.
#[tokio::test]
async fn the_server_orders_sealed_submissions_and_a_seal_trims_them() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    let account = phone.register("phone@example.com").await;
    let organization = phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let first = author.compose(b"saved\n", 1_000, None);
    let _ = phone
        .publish(&author, block, first.0, first.1, None, 1_000)
        .await;
    phone.join_session(block).await;
    let token = phone.token.clone();
    let mut laptop = harness.second_connection(&token, organization).await;
    laptop.join_session(block).await;

    phone
        .submit(block, 1, 1, 0, b"sealed one".to_vec())
        .await
        .unwrap();
    laptop
        .submit(block, 2, 1, 0, b"sealed two".to_vec())
        .await
        .unwrap();
    // Sent again, as after a lost answer: answered, not numbered again.
    phone
        .submit(block, 1, 1, 0, b"sealed one".to_vec())
        .await
        .unwrap();
    for client in [&mut phone, &mut laptop] {
        let one = client.sequenced(block).await;
        let two = client.sequenced(block).await;
        assert_eq!(
            [(one.sequence, one.origin), (two.sequence, two.origin)],
            [(1, 1), (2, 2)],
            "every participant hears one order"
        );
        assert_eq!(
            one.payload, b"sealed one",
            "the server passes the bytes on as they were"
        );
        assert!(one.received.is_some());
    }

    let journal = phone
        .journal_of(block)
        .await
        .expect("the journal holds the order");
    assert_eq!(
        journal.base.commit,
        Some(first.0),
        "a journal starts on the head"
    );
    assert_eq!(journal.entries.len(), 2);

    let second = author.compose(b"sealed through one\n", 2_000, Some(first.0));
    let sealed = phone
        .seal(
            &author,
            block,
            second.0,
            second.1,
            Some(first.0),
            2_000,
            Some(of_build(JournalBase {
                commit: Some(second.0),
                sequence: 1,
                state: Vec::new(),
            })),
        )
        .await;
    assert!(
        matches!(sealed, Ok(be_block_protocol::PublishOutcome::Published(_))),
        "{sealed:?}"
    );
    let heard = loop {
        if let Notice::Sealed {
            block: of,
            commit,
            through,
        } = laptop.notification().await
            && of == block
        {
            break (commit, through);
        }
    };
    assert_eq!(
        heard,
        (second.0, 1),
        "every participant hears which head holds what"
    );
    let journal = phone.journal_of(block).await.expect("one entry is left");
    assert_eq!(
        journal
            .entries
            .iter()
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>(),
        [2],
        "the seal holds the first"
    );
    laptop
        .submit(block, 2, 2, 2, b"sealed three".to_vec())
        .await
        .unwrap();
    assert_eq!(
        laptop.sequenced(block).await.sequence,
        3,
        "the order goes on"
    );
}
