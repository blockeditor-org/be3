use super::*;

/// A batch whose caller lets go of it before it ends (a connection that closes while its write
/// runs) rolls back, and leaves the database to the writes after it, which are kept.
#[tokio::test]
async fn a_write_let_go_of_midway_keeps_nothing_and_leaves_no_transaction_open() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let account = owner.id();
    owner.personal().await;
    // A second store on the server's database, whose batch holds its write lock as one the
    // server ran would.
    let store = ServerStore::open(&harness.directory).unwrap();
    let (wrote, written) = tokio::sync::oneshot::channel::<()>();
    let batch = {
        let store = store.clone();
        tokio::spawn(async move {
            store
                .batch(async {
                    store
                        .with_database(move |_, database| {
                            database.execute(
                                "UPDATE accounts SET display_name = 'let go of' WHERE id = ?1",
                                [account.to_string()],
                            )?;
                            Ok(())
                        })
                        .await?;
                    let _ = wrote.send(());
                    std::future::pending::<Result<(), ServerError>>().await
                })
                .await
        })
    };
    written.await.expect("the batch wrote");
    batch.abort();
    let _ = batch.await;

    // The server's next write waits for the lock the batch held, then is kept.
    let (block, create) = owner.new_block_op(BlockParent::Root).await;
    let made = owner.call(|peer| peer.write::<BlockSummary>(create)).await;
    assert!(made.is_ok(), "{made:?}");
    let database = metadata_database(&harness.directory);
    let name: String = database
        .query_row(
            "SELECT display_name FROM accounts WHERE id = ?1",
            [account.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_ne!(name, "let go of", "the batch let go of was kept");
    let kept: bool = database
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM blocks WHERE id = ?1)",
            [block.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(kept, "the write after the batch was not kept");
    // And the store that held the batch writes again with no transaction left open.
    let free = store
        .with_database(|_, database| Ok(database.is_autocommit()))
        .await
        .unwrap();
    assert!(free, "the batch let go of left its transaction open");

    harness.stop().await;
}
