use super::*;

/// One person's devices compare the heads of signed logs they kept through the account's own
/// block, even when they share no session or commit (guides/blocks/keys.md, "Forks"): each
/// shares its heads there, signed, and compares those the others shared. The laptop shares its
/// head before the server rewrites the log; the phone agrees with it, and the tablet, shown the
/// rewritten log, finds the fork.
#[tokio::test]
async fn an_accounts_devices_compare_log_heads_through_its_block() {
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
    laptop.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(
        laptop.trade_account_heads().await.unwrap(),
        Vec::new(),
        "nothing to compare before anything is shared"
    );

    let phone = another(&harness, &laptop).await;
    phone.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(
        phone.trade_account_heads().await.unwrap(),
        Vec::new(),
        "the same head as the one shared is compared with nothing asked"
    );

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
    let tablet = another(&harness, &laptop).await;
    tablet.read_log(chat, 0, 10).await.unwrap();
    let compared = tablet.trade_account_heads().await.unwrap();
    assert!(
        matches!(
            compared.as_slice(),
            [(block, LogAgreement::Forked(fork))]
                if *block == chat
                    && fork.theirs.size == 2
                    && matches!(fork.from, crate::HeadFrom::Account { .. })
        ),
        "the tablet took the head the laptop shared as agreeing: {compared:?}"
    );
    assert!(tablet.log_fork(chat).is_some(), "the verdict is kept");
    harness.stop().await;
}
