use super::*;

/// A replica that joins its block's session tells the participants the head of the block's
/// signed log its device kept, and compares a head a participant tells it with its own
/// (guides/blocks/keys.md, "Forks"). The phone read the log before the server rewrote it, the
/// laptop after: the phone hears the laptop's head as the laptop's replica joins, and the
/// laptop, told the phone's, finds the fork and keeps it as the block's verdict.
#[test]
fn a_replica_joining_its_session_trades_log_heads_with_the_participants() {
    with_another_device(|laptop, phone, shared, mut replicas| async move {
        let block = laptop
            .create::<Ledger>(BlockParent::Root)
            .await
            .expect("the ledger is made");
        laptop
            .save(block, &Ledger::default(), None)
            .await
            .expect("the empty ledger is published");
        for said in [&b"one"[..], b"two"] {
            laptop
                .append(block, "test.app", said, be_block_content::Along::default())
                .await
                .expect("the entry is appended");
        }
        let before = phone.read_log(block, 0, 10).await.expect("the log reads").head;
        rewrite_log(laptop.organization(), block, laptop.account(), 3);
        let after = laptop.read_log(block, 0, 10).await.expect("the log reads").head;
        assert_eq!((before.size, after.size), (2, 3));

        let mut on_phone = phone.connection().subscribe();
        phone
            .connection()
            .join_session(phone.organization(), block, be_block_store::Hash::of(b"a phone build"))
            .await
            .expect("the phone joins the session");
        replicas
            .open((&laptop, &shared), block, Ledger::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            step(&laptop, &shared, &mut replicas).await;
        }
        let told = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                if let be_block_client::Event::Relayed {
                    block: relayed,
                    payload,
                    ..
                } = on_phone.recv().await.expect("the phone hears the session")
                    && relayed == block
                    && let Ok(be_block_client::Relayed::LogHead(signed)) =
                        phone.unseal(block, &payload)
                {
                    break (signed.account, signed.head);
                }
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "the replica told no head as it joined: {:?}",
                shared.lock().unwrap().errors
            )
        });
        assert_eq!(told, (laptop.account(), after));

        phone
            .tell_log_head(block, None)
            .await
            .expect("the phone tells its head")
            .expect("the phone kept a head")
            .await
            .expect("the session takes it");
        let found = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                if let Some(fork) = laptop.log_fork(block) {
                    break fork;
                }
                step(&laptop, &shared, &mut replicas).await;
            }
        })
        .await
        .expect("the laptop compares the head it heard");
        assert_eq!((found.ours.size, found.theirs.size), (3, 2));
        assert!(matches!(
            found.from,
            be_block_client::HeadFrom::Session { account, .. } if account == phone.account()
        ));
    });
}

/// The server forgets `block`'s log and makes up one of `count` entries in its place, as a
/// server showing devices different logs would.
fn rewrite_log(organization: Uuid, block: Uuid, author: Uuid, count: usize) {
    let objects = SERVER_OBJECTS.take().expect("a server is running");
    let database = be_block_sqlite_schema::open(
        objects
            .parent()
            .expect("the objects lie in the server's directory")
            .join("metadata.sqlite"),
    )
    .expect("the server's database opens");
    SERVER_OBJECTS.set(Some(objects));
    be_server::log::forget(&database, organization, block).expect("the log is forgotten");
    let entries = (0..count)
        .map(|index| be_block_protocol::NewEntry {
            id: Uuid::new_v4(),
            payload: format!("made up {index}").into_bytes(),
            signature: vec![1; 64],
        })
        .collect();
    be_server::log::append(
        &database,
        organization,
        block,
        author,
        entries,
        be_block_commit::now_milliseconds(),
    )
    .expect("the made-up log is appended");
}
