use super::*;

/// One person's devices compare the heads of signed logs they kept through the account's own
/// block, even when they share no session or commit (guides/blocks/keys.md, "Forks"): each
/// shares its heads there, signed, and compares those the others shared. The laptop shares its
/// head before the server rewrites the log; the phone agrees with it, and the tablet, shown the
/// rewritten log, finds the fork and leaves the laptop's head where it is, though its own is
/// longer.
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
    assert!(
        laptop.trade_account_heads().await.unwrap().is_empty(),
        "nothing to compare before anything is shared"
    );
    let shared = laptop
        .account_log_head(laptop.organization(), chat)
        .await
        .expect("the laptop shares its head");
    assert_eq!(shared.size, 2);

    let phone = another(&harness, &laptop).await;
    phone.read_log(chat, 0, 10).await.unwrap();
    assert!(
        phone.trade_account_heads().await.unwrap().is_empty(),
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
            [(block, Ok(LogAgreement::Forked(fork)))]
                if *block == chat
                    && fork.theirs.size == 2
                    && matches!(fork.from, crate::HeadFrom::Account { .. })
        ),
        "the tablet, shown the rewritten log, did not find the fork in the head the laptop \
         shared: {compared:?}"
    );
    assert!(tablet.log_fork(chat).is_some(), "the verdict is kept");
    assert_eq!(
        tablet.account_log_head(laptop.organization(), chat).await,
        Some(shared),
        "a head found forked stays as evidence, never moved forward over"
    );
    harness.stop().await;
}
