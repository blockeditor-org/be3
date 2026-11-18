use super::*;

/// A connection switching a new database to WAL while another writes it (as the other's own
/// switch does) waits for that write instead of failing with `SQLITE_BUSY`.
#[test]
fn wal_waits_out_another_connection_writing_the_new_database() {
    let directory = std::env::temp_dir().join(format!("be-sqlite-wal-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("new.sqlite");
    let _ = std::fs::remove_file(&path);
    let writer = open(&path).unwrap();
    writer
        .execute_batch("BEGIN IMMEDIATE; CREATE TABLE notes (body TEXT);")
        .unwrap();
    let switching = open(&path).unwrap();
    assert!(
        switching
            .pragma_update(None, "journal_mode", "WAL")
            .is_err(),
        "SQLite no longer refuses this switch at once; use_wal may be unneeded"
    );
    let committing = std::thread::spawn(move || writer.execute_batch("COMMIT").unwrap());

    use_wal(&switching).unwrap();

    committing.join().unwrap();
    let mode: String = switching
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    drop(switching);
    std::fs::remove_dir_all(&directory).unwrap();
}
