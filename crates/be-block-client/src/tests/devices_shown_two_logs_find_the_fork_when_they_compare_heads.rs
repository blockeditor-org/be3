use super::*;

/// A server that shows two devices two logs from the start is caught only when they compare the
/// heads they kept (guides/blocks/keys.md, "Forks"): a head the server proves lies on the log
/// this device reads, as long or shorter, agrees; one it cannot is a fork, kept as the block's
/// verdict and told once.
#[tokio::test]
async fn devices_shown_two_logs_find_the_fork_when_they_compare_heads() {
    let harness = Harness::start().await;
    let laptop = harness.owner("owner@example.com").await;
    let chat = workspace(&laptop, "Chat").await;
    let append = |text: &'static str| {
        let laptop = &laptop;
        async move {
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
    };
    append("one").await;
    append("two").await;
    let two = laptop.read_log(chat, 0, 10).await.unwrap().head;
    append("three").await;
    let three = laptop.read_log(chat, 0, 10).await.unwrap().head;
    assert_eq!((two.size, three.size), (2, 3));

    // The phone reads the same log: the laptop's heads, now and before, lie on it.
    let phone = another(&harness, &laptop).await;
    let from = HeadFrom::Session {
        account: laptop.account(),
    };
    for head in [three, two] {
        assert_eq!(
            phone.compare_log_head(chat, head, from).await.unwrap(),
            LogAgreement::Agrees,
            "a head of the same log was taken for a fork"
        );
    }
    assert_eq!(phone.log_fork(chat), None);

    // The server rewrites the log, so a device that reads it now is shown another one, which
    // is consistent with itself. The laptop's head does not lie on it.
    {
        let database = harness.database();
        be_server::log::forget(&database, laptop.organization(), chat).unwrap();
        let entries = (0..4)
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
    let tablet = another(&harness, &laptop).await;
    let mut events = tablet.connection().subscribe();
    let shown = tablet.read_log(chat, 0, 10).await.unwrap().head;
    assert_eq!(shown.size, 4, "the tablet reads the rewritten log");
    let LogAgreement::Forked(fork) = tablet.compare_log_head(chat, three, from).await.unwrap()
    else {
        panic!("a head of another log was taken as agreeing");
    };
    assert_eq!(
        (fork.block, fork.ours.size, fork.theirs.size, fork.from),
        (chat, 4, 3, from)
    );
    assert_eq!(tablet.log_fork(chat), Some(fork.clone()), "the verdict is kept");
    // A longer head the server never served this device cannot lie on its log either; the
    // verdict kept stays the first, told once.
    let longer = be_block_protocol::LogHead {
        size: 9,
        root: be_block_store::Hash::from_bytes([7; 32]),
    };
    assert_eq!(
        tablet.compare_log_head(chat, longer, from).await.unwrap(),
        LogAgreement::Forked(fork.clone())
    );
    let mut told = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let crate::Event::LogForked(fork) = event {
            told.push(fork);
        }
    }
    assert_eq!(told, vec![fork], "the fork is told once, as it is first found");
    harness.stop().await;
}
