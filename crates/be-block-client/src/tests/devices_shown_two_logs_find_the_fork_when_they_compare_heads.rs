use super::*;

/// A server that shows two devices two logs from the start is caught only when they compare the
/// heads they kept (guides/blocks/keys.md, "Forks"). A head is taken only signed with a key the
/// device verified for its account, so the server can neither make one up nor change one. A
/// head the server proves lies on the log this device reads, as long or shorter, agrees; one it
/// proves does not is a fork, kept as the block's verdict with the server's root at that size
/// as evidence, and told once. A head of the same size with another root is a fork with no
/// proof needed, and a cut of this device's own log is kept as the same verdict.
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
    laptop.read_log(chat, 0, 10).await.unwrap();
    let two = laptop.signed_log_head(chat).unwrap();
    append("three").await;
    laptop.read_log(chat, 0, 10).await.unwrap();
    let three = laptop.signed_log_head(chat).unwrap();
    assert_eq!((two.head.size, three.head.size), (2, 3));
    let signed = |size: u64, root: [u8; 32]| SignedLogHead {
        account: laptop.account(),
        head: be_block_protocol::LogHead {
            size,
            root: be_block_store::Hash::from_bytes(root),
        },
        signature: laptop
            .keys()
            .identity()
            .sign_log_head(laptop.organization().into_bytes(), chat.into_bytes(), size, &root),
    };

    // The phone reads the same log: the laptop's heads, now and before, lie on it.
    let phone = another(&harness, &laptop).await;
    for head in [three.clone(), two.clone()] {
        assert_eq!(
            phone
                .compare_log_head(chat, head, HeadVia::Session)
                .await
                .unwrap(),
            LogAgreement::Agrees,
            "a head of the same log was taken for a fork"
        );
    }
    // What the server makes up is taken for nothing: a head signed by no key the phone
    // verified, or another root under the laptop's signature.
    let mut unsigned = three.clone();
    unsigned.signature = vec![0; 64];
    let mut changed = three.clone();
    changed.head.root = be_block_store::Hash::from_bytes([5; 32]);
    for made_up in [unsigned, changed] {
        assert!(
            matches!(
                phone.compare_log_head(chat, made_up, HeadVia::Session).await,
                Err(ClientError::Forged(block)) if block == chat
            ),
            "a head the account never signed was compared"
        );
    }
    assert!(
        matches!(
            phone
                .compare_log_head(chat, signed(0, [6; 32]), HeadVia::Session)
                .await,
            Err(ClientError::Tampered(_))
        ),
        "an empty head with a root was taken"
    );
    // The empty head lies on every log: nothing the server answers about it is asked, so none
    // can plant a fork with it.
    assert_eq!(
        phone
            .compare_log_head(
                chat,
                signed(0, be_block_keys::merkle::empty_root()),
                HeadVia::Session
            )
            .await
            .unwrap(),
        LogAgreement::Agrees
    );
    assert_eq!(phone.log_fork(chat), None, "nothing made up is kept");

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
    let LogAgreement::Forked(fork) = tablet
        .compare_log_head(chat, three.clone(), HeadVia::Session)
        .await
        .unwrap()
    else {
        panic!("a head of another log was taken as agreeing");
    };
    assert_eq!(
        (fork.block, fork.ours.size, fork.theirs.size),
        (chat, 4, 3)
    );
    assert_eq!(
        fork.from,
        crate::HeadFrom::Session {
            account: laptop.account(),
            signature: three.signature.clone(),
        }
    );
    assert!(
        fork.prefix
            .is_some_and(|prefix| prefix != *three.head.root.as_bytes()),
        "the server's root at their size is kept as evidence"
    );
    assert!(
        be_block_keys::merkle::consistent(
            3,
            &fork.prefix.unwrap(),
            4,
            &fork.ours.root,
            &fork.proof
        ),
        "with the proof that the head this device read extends it"
    );
    assert_eq!(fork.kind, crate::ForkKind::Forked);
    assert_eq!(tablet.log_fork(chat), Some(fork.clone()), "the verdict is kept");
    // A longer head, or one as long with another root, cannot lie on its log either; the
    // verdict kept stays the first, told once.
    for other in [signed(9, [7; 32]), signed(4, [8; 32])] {
        assert_eq!(
            tablet
                .compare_log_head(chat, other, HeadVia::Session)
                .await
                .unwrap(),
            LogAgreement::Forked(fork.clone())
        );
    }
    let mut told = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let crate::Event::LogForked(fork) = event {
            told.push(fork);
        }
    }
    assert_eq!(told, vec![fork], "the fork is told once, as it is first found");

    // A head as long with another root is a fork on a device with no verdict yet too.
    let pad = another(&harness, &laptop).await;
    pad.read_log(chat, 0, 10).await.unwrap();
    assert!(matches!(
        pad.compare_log_head(chat, signed(4, [8; 32]), HeadVia::Session)
            .await
            .unwrap(),
        LogAgreement::Forked(fork) if fork.prefix.is_none() && fork.theirs.size == 4
    ));

    // The laptop, served the rewritten log against the head it kept, cannot be shown it
    // extends that head: the server's proof does not check, kept as its verdict.
    assert!(matches!(
        laptop.read_log(chat, 0, 10).await,
        Err(ClientError::Tampered(_))
    ));
    assert!(matches!(
        laptop.log_fork(chat),
        Some(fork) if fork.from == crate::HeadFrom::Server
            && fork.kind == crate::ForkKind::Misbehaved
            && fork.ours.size == 3
    ));
    harness.stop().await;
}
