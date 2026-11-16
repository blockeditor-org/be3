use super::*;

use crate::backup::{self, BackupKey, Directory};

/// Whoever can write the backup storage could put an older snapshot, from before a revocation,
/// under a newer name for a restore to pick; its seal binds the name it was made under, so it
/// opens nothing as another.
#[tokio::test]
async fn an_older_backup_renamed_as_the_newest_does_not_restore() {
    let harness = Harness::start().await;
    let target = Directory(harness.directory.with_extension("backup"));
    let key = BackupKey::generate();
    let older = tokio::task::spawn_blocking({
        let (target, key) = (
            Directory(target.0.clone()),
            BackupKey::from_hex(&key.to_hex()).unwrap(),
        );
        let data_dir = harness.directory.clone();
        move || backup::backup(&data_dir, None, &target, &key, std::time::SystemTime::now())
    })
    .await
    .unwrap()
    .expect("the backup runs");

    let databases = target.0.join("databases");
    std::fs::copy(
        databases.join(&older.snapshot),
        databases.join("zzzz.sealed"),
    )
    .unwrap();
    let restored = harness.directory.with_extension("restored");
    assert!(
        backup::restore(&target, &key, &restored, None, None).is_err(),
        "a snapshot renamed as the newest restored"
    );
    assert_eq!(
        backup::restore(&target, &key, &restored, None, Some(&older.snapshot))
            .expect("it restores under its own name")
            .snapshot,
        older.snapshot
    );
}
