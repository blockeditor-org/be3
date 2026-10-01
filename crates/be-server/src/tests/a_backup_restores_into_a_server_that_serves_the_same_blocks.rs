use super::*;

use crate::backup::{self, BackupKey, Directory};

#[tokio::test]
async fn a_backup_restores_into_a_server_that_serves_the_same_blocks() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("keeper@example.com").await;
    let workspace = client.workspace("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;
    let (head, chunks) = author.compose(b"a note worth keeping\n", 1_000, None);
    client
        .publish(&author, block, head, chunks.clone(), None, 1_000)
        .await;

    let target = Directory(harness.directory.with_extension("backup"));
    let key = BackupKey::generate();
    let data_dir = harness.directory.clone();
    let report = tokio::task::spawn_blocking({
        let target = Directory(target.0.clone());
        let key = BackupKey::from_hex(&key.to_hex()).unwrap();
        move || backup::backup(&data_dir, &target, &key, std::time::SystemTime::now())
    })
    .await
    .unwrap()
    .expect("the backup runs while the server is up");
    assert!(report.uploaded > 0);
    assert_eq!(report.uploaded, report.objects);

    let again = backup::backup(
        &harness.directory,
        &target,
        &key,
        std::time::SystemTime::now() + Duration::from_secs(3600),
    )
    .unwrap();
    assert_eq!(
        again.uploaded, 0,
        "a second backup uploaded objects it had already sent"
    );

    let restored = harness.directory.with_extension("restored");
    assert!(
        backup::restore(&target, &BackupKey::generate(), &restored, None).is_err(),
        "another key opened the backup"
    );
    let report = backup::restore(&target, &key, &restored, None).expect("the backup restores");
    assert_eq!(
        report.snapshot, again.snapshot,
        "restore picked an older snapshot"
    );
    assert!(
        backup::restore(&target, &key, &restored, None).is_err(),
        "a restore wrote over a server"
    );
    assert!(backup::verify(&restored).unwrap().missing.is_empty());

    let token = client.token.clone();
    harness.stop().await;
    let revived = Harness::at(restored.clone()).await;
    let mut returning = revived.second_connection(&token, workspace).await;
    assert_eq!(returning.read_block(block).await.head, Some(head));
    for chunk in &chunks {
        let response = returning
            .send(|request| ClientMessage::GetObject {
                request,
                hash: *chunk,
            })
            .await;
        assert!(
            matches!(response, ServerMessage::Object { bytes: Some(_), .. }),
            "a chunk did not survive the restore"
        );
    }
    revived.stop().await;
    let _ = std::fs::remove_dir_all(&target.0);
}
