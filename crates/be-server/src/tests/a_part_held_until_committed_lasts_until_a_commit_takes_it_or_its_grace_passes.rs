use super::*;

#[tokio::test]
async fn a_part_held_until_committed_lasts_until_a_commit_takes_it_or_its_grace_passes() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("indexer@example.com").await;
    let organization = client.organization("index").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let vault = author.commits.vault();
    let taken = vault
        .write(KEY, CONTENT, b"a part a commit takes".as_slice())
        .unwrap();
    let left = vault
        .write(KEY, CONTENT, b"a part no commit takes".as_slice())
        .unwrap();
    let taken_objects = vault.objects(&taken).unwrap();
    let left_objects = vault.objects(&left).unwrap();
    for objects in [&taken_objects, &left_objects] {
        client.upload(&author.store, objects).await;
        for _ in 0..2 {
            let held = client
                .call(|peer| {
                    client::objects::hold_objects(
                        peer,
                        block,
                        objects.clone(),
                        Hold::UntilCommitted,
                    )
                })
                .await;
            assert!(matches!(held, Ok(())), "{held:?}");
        }
    }

    let commit = author
        .commits
        .put(KEY, &Commit::new(taken, account, 1_000))
        .unwrap();
    let published = client
        .publish(&author, block, commit, taken_objects.clone(), None, 1_000)
        .await;
    assert!(
        matches!(
            published,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{published:?}"
    );
    let directory = harness.directory.clone();
    let database = rusqlite::Connection::open(directory.join("metadata.sqlite")).unwrap();
    let holding: i64 = database
        .query_row(
            "SELECT count(*) FROM block_objects WHERE block_id = ?1",
            [block.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        holding,
        left_objects.len() as i64,
        "the commit took the hold of what it uses, and holding twice held once"
    );
    drop(database);
    drop(client);

    let identity = Identity {
        account,
        organization,
        role: OrganizationRole::Editor,
    };
    let left_bytes: u64 = left_objects
        .iter()
        .map(|hash| author.store.len(*hash).unwrap().unwrap() as u64)
        .sum();
    // Another store over the same directory, whose grace has passed for every hold.
    let impatient = ServerStore::open_with(
        &directory,
        crate::store::StorageLimits {
            upload_grace: Duration::ZERO,
            ..crate::store::StorageLimits::DEFAULT
        },
        None,
    )
    .unwrap();
    let before = impatient.storage_usage(identity).await.unwrap().held;
    assert_eq!(
        impatient.collect_uploads().await.unwrap(),
        left_objects.len(),
        "a hold no commit took ends once the grace passes"
    );
    for hash in &left_objects {
        assert!(!impatient.objects().has(*hash).unwrap());
    }
    for hash in &taken_objects {
        assert!(impatient.objects().has(*hash).unwrap());
    }
    assert_eq!(
        impatient.storage_usage(identity).await.unwrap().held,
        before - left_bytes,
        "the block stops being charged for what it no longer holds"
    );

    drop(impatient);
    harness.stop().await;
}
