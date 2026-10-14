use super::*;

#[test]
fn a_new_table_index_and_defaulted_column_are_added_in_place() {
    let connection = Connection::open_in_memory().unwrap();
    ensure(&connection, FIRST).unwrap();
    connection
        .execute("INSERT INTO notes (id, body) VALUES ('a', 'kept')", [])
        .unwrap();

    let grown = "
        CREATE TABLE notes (
            id TEXT PRIMARY KEY,
            body TEXT NOT NULL,
            pinned INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE tags (note TEXT NOT NULL, tag TEXT NOT NULL);
        CREATE INDEX tags_by_tag ON tags (tag);
    ";
    ensure(&connection, grown).unwrap();
    ensure(&connection, grown).unwrap();

    let (body, pinned): (String, i64) = connection
        .query_row("SELECT body, pinned FROM notes WHERE id = 'a'", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!((body.as_str(), pinned), ("kept", 0));
    let index: String = connection
        .query_row(
            "SELECT tbl_name FROM sqlite_master WHERE name = 'tags_by_tag'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(index, "tags");
}
