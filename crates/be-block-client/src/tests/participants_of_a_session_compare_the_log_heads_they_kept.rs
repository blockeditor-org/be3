use super::*;

use crate::Event;

/// The head of a block's signed log a device kept travels in the block's live session, sealed
/// under the block's key and framed apart from what its type module relays
/// (`Peer::tell_log_head`), and a participant that hears it compares it with its own
/// (guides/blocks/keys.md, "Forks"): the laptop, shown the log before the server rewrote it,
/// tells its head; the tablet, shown the rewritten log, finds the fork.
#[tokio::test]
async fn participants_of_a_session_compare_the_log_heads_they_kept() {
    let harness = Harness::start().await;
    let laptop = harness.owner("owner@example.com").await;
    let chat = workspace(&laptop, "Chat").await;
    for text in ["one", "two"] {
        laptop
            .append(
                chat,
                "test.app",
                text.as_bytes(),
                be_block_content::Along::default(),
            )
            .await
            .unwrap();
    }
    let kept = laptop.read_log(chat, 0, 10).await.unwrap().head;
    {
        let database = harness.database();
        be_server::log::forget(&database, laptop.organization(), chat).unwrap();
        let entries = (0..3)
            .map(|index| be_block_protocol::NewEntry {
                id: Uuid::new_v4(),
                payload: format!("made up {index}").into_bytes(),
                signature: vec![1; 64],
            })
            .collect();
        be_server::log::append(
            &database,
            laptop.organization(),
            chat,
            laptop.account(),
            entries,
            be_block_commit::now_milliseconds(),
        )
        .unwrap();
    }
    let tablet = harness.second(&laptop).await;
    tablet.read_log(chat, 0, 10).await.unwrap();
    let mut on_tablet = tablet.connection().subscribe();
    for device in [&laptop, &tablet] {
        device
            .connection()
            .join_session(device.organization(), chat, BUILD)
            .await
            .unwrap();
    }
    laptop
        .tell_log_head(chat, None)
        .await
        .unwrap()
        .expect("the laptop kept a head of the chat's log")
        .await
        .unwrap();
    let (account, head) = loop {
        if let Event::Relayed { block, payload, .. } = on_tablet.recv().await.unwrap()
            && block == chat
        {
            match tablet.unseal(chat, &payload).unwrap() {
                crate::Relayed::LogHead { account, head } => break (account, head),
                other => panic!("the laptop's head was read as {other:?}"),
            }
        }
    };
    assert_eq!((account, head), (laptop.account(), kept));
    let from = HeadFrom::Session { account };
    assert!(
        matches!(
            tablet.compare_log_head(chat, head, from).await.unwrap(),
            LogAgreement::Forked(fork) if fork.theirs.size == 2 && fork.ours.size == 3
        ),
        "the tablet took a head of another log as agreeing"
    );
    assert!(
        harness
            .second(&laptop)
            .await
            .tell_log_head(chat, None)
            .await
            .unwrap()
            .is_none(),
        "a device that never read the log tells no head"
    );
    harness.stop().await;
}
