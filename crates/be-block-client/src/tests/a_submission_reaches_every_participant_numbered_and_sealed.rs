use be_block_session::{OpId, Submission};
use be_type_example_counter::CounterContent;

use super::*;
use crate::{Event, peer};

/// A submission goes to the server sealed with the block's key; the server numbers it and every
/// participant hears it, its author too, which opens it; offline, the device numbers it in its
/// own order after what it heard.
#[tokio::test]
async fn a_submission_reaches_every_participant_numbered_and_sealed() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.second(&phone).await;
    let mut on_phone = phone.connection().subscribe();
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
        .submit(block, BUILD, submission, true)
        .await
    else {
        panic!("an online submission goes to the server");
    };
    answer.await.unwrap();
    for (peer, events) in [(&phone, &mut on_phone), (&laptop, &mut on_laptop)] {
        let entry = loop {
            if let Event::Sequenced { block: of, entry } = events.recv().await.unwrap()
                && of == block
            {
                break entry;
            }
        };
        assert_eq!(entry.sequence, 1);
        assert_ne!(
            entry.payload, b"an edit",
            "the entry crossed the server sealed"
        );
        peer.heard_entry(block, BUILD, entry.clone()).unwrap();
        assert_eq!(peer.open_entry(block, entry).unwrap().payload, b"an edit");
    }

    let offline = Submission {
        id: OpId {
            origin: 1,
            counter: 2,
        },
        base: 1,
        barrier: false,
        payload: b"made offline".to_vec(),
    };
    let Ok(peer::Submitted::Offline(entry)) =
        phone.submit(block, BUILD, offline, false).await
    else {
        panic!("an offline submission is numbered on the device");
    };
    assert_eq!((entry.sequence, entry.received), (2, None));
    assert_eq!(entry.payload, b"made offline");
}
