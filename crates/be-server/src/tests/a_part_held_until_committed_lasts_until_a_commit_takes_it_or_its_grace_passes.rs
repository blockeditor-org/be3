use be_block_protocol::{JournalBase, JournalEntry};
use rusqlite::{OptionalExtension, params};

use super::*;

async fn hold(client: &mut TestClient, block: Uuid, objects: &[Hash], hold: Hold) {
    let objects = objects.to_vec();
    let held = client
        .call(|peer| client::objects::hold_objects(peer, block, objects, hold))
        .await;
    assert!(matches!(held, Ok(())), "{held:?}");
}

async fn held(directory: &std::path::Path, identity: Identity) -> u64 {
    ServerStore::open(directory)
        .unwrap()
        .storage_usage(identity)
        .await
        .unwrap()
        .held
}

#[tokio::test]
async fn a_part_held_until_committed_lasts_until_a_commit_takes_it_or_its_grace_passes() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("indexer@example.com").await;
    let organization = client.organization("index").await;
    let identity = Identity {
        account,
        organization,
        role: OrganizationRole::Editor,
        bound: None,
    };
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;
    let other = client.create_block(BlockParent::Root).await;

    let vault = author.commits.vault();
    let part = |text: &[u8]| {
        let manifest = vault.write(KEY, CONTENT, text).unwrap();
        let objects = vault.objects(&manifest).unwrap();
        (manifest, objects)
    };
    let (taken, taken_objects) = part(b"a part a commit takes");
    let (_, left_objects) = part(b"a part no commit takes");
    let (_, kept_objects) = part(b"a part held again for as long as the block lasts");
    let (_, shared_objects) = part(b"a part another block holds for good");
    let (_, named_objects) = part(b"a part an operation in the journal names");
    let bytes_of = |objects: &[Hash]| -> u64 {
        objects
            .iter()
            .map(|hash| author.store.len(*hash).unwrap().unwrap() as u64)
            .sum()
    };
    for objects in [
        &taken_objects,
        &left_objects,
        &kept_objects,
        &shared_objects,
        &named_objects,
    ] {
        client.upload(&author.store, objects).await;
    }
    for objects in [&taken_objects, &left_objects, &left_objects, &kept_objects] {
        hold(&mut client, block, objects, Hold::UntilCommitted).await;
    }
    hold(&mut client, block, &kept_objects, Hold::Lasting).await;
    hold(&mut client, block, &shared_objects, Hold::UntilCommitted).await;
    hold(&mut client, other, &shared_objects, Hold::Lasting).await;
    hold(&mut client, other, &named_objects, Hold::UntilCommitted).await;

    let directory = harness.directory.clone();
    let before = held(&directory, identity).await;
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
    assert_eq!(
        held(&directory, identity).await,
        before + bytes_of(&[commit.hash()]),
        "a commit taking what the block held changes the charge only by the commit itself"
    );
    let database = metadata_database(&directory);
    let held_since = |hash: Hash| -> Option<Option<i64>> {
        database
            .query_row(
                "SELECT held_since FROM block_objects WHERE block_id = ?1 AND hash = ?2",
                params![block.to_string(), hash.to_hex()],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
    };
    for hash in &taken_objects {
        assert_eq!(held_since(*hash), None, "the commit took the hold");
    }
    for hash in &kept_objects {
        assert_eq!(
            held_since(*hash),
            Some(None),
            "a lasting hold outlasts the other"
        );
    }

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
    let generation = 1;
    let tag = Hash::from_bytes([3; 32]);
    impatient
        .append_journal(
            organization,
            other,
            generation,
            tag,
            vec![JournalEntry {
                generation,
                sequence: 1,
                payload: b"an operation naming a part".to_vec(),
            }],
        )
        .await
        .unwrap();

    let before = impatient.storage_usage(identity).await.unwrap().held;
    assert_eq!(
        impatient.collect_uploads().await.unwrap(),
        left_objects.len(),
        "a hold no commit took ends once the grace passes, and only that"
    );
    let has = |hash: &Hash| impatient.objects().has(*hash).unwrap();
    assert!(!left_objects.iter().any(has));
    for objects in [
        &taken_objects,
        &kept_objects,
        &shared_objects,
        &named_objects,
    ] {
        assert!(objects.iter().all(has));
    }
    assert_eq!(
        held_since(shared_objects[0]),
        None,
        "the block's hold ended though another block keeps the part"
    );
    assert_eq!(
        impatient.storage_usage(identity).await.unwrap().held,
        before - bytes_of(&left_objects),
        "the block stops being charged for what it no longer holds"
    );

    impatient
        .rebase_journal(
            organization,
            other,
            generation,
            tag,
            JournalBase {
                commit: None,
                sequence: 1,
                state: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        impatient.collect_uploads().await.unwrap(),
        named_objects.len(),
        "a part the journal named ends once the journal no longer holds the operation"
    );
    assert!(!named_objects.iter().any(has));

    drop(database);
    drop(impatient);
    drop(client);
    harness.stop().await;
}
