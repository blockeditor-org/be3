use super::*;

/// A reader holding a rollback-mode database (an older build, the sqlite3 shell) keeps the
/// switch to WAL from ever taking: the write lock the switch waits on coexists with that read.
/// The switch answers `SQLITE_BUSY` rather than spinning.
#[test]
fn wal_refuses_rather_than_spins_while_a_reader_holds_the_old_journal() {
    let directory =
        std::env::temp_dir().join(format!("be-sqlite-wal-reader-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("old.sqlite");
    let _ = std::fs::remove_file(&path);
    let reader = open(&path).unwrap();
    reader
        .execute_batch("CREATE TABLE notes (body TEXT); BEGIN; SELECT * FROM notes;")
        .unwrap();
    let switching = open(&path).unwrap();
    // The busy handler's wait is the connection's own setting; a short one keeps the test quick.
    switching
        .busy_timeout(std::time::Duration::from_millis(10))
        .unwrap();

    let switched = use_wal(&switching);

    assert!(
        matches!(
            &switched,
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == rusqlite::ErrorCode::DatabaseBusy
        ),
        "a switch the reader blocks answers busy: {switched:?}"
    );
    reader.execute_batch("COMMIT").unwrap();
    drop((reader, switching));
    std::fs::remove_dir_all(&directory).unwrap();
}
