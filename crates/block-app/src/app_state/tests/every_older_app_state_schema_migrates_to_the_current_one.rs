use rusqlite::Connection;

use super::*;
use crate::app_state::native::{MIGRATIONS, migrate};

fn schema_of(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn version_of(connection: &Connection) -> usize {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

#[test]
fn every_older_app_state_schema_migrates_to_the_current_one() {
    let directory = std::env::temp_dir().join(format!("block-app-state-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();

    let fresh_path = directory.join("fresh.sqlite3");
    drop(AppStateStore::open(&fresh_path).unwrap());
    let fresh = Connection::open(&fresh_path).unwrap();
    assert_eq!(version_of(&fresh), MIGRATIONS.len());

    let older = (0..MIGRATIONS.len()).map(Some).chain([None]);
    for (index, applied) in older.enumerate() {
        let path = directory.join(format!("older-{index}.sqlite3"));
        let connection = Connection::open(&path).unwrap();
        match applied {
            Some(applied) => migrate(&connection, &MIGRATIONS[..applied]).unwrap(),
            None => connection.execute_batch(MIGRATIONS[0]).unwrap(),
        }
        drop(connection);
        let store = AppStateStore::open(&path).unwrap();
        assert!(store.accounts().unwrap().is_empty());
        drop(store);
        let connection = Connection::open(&path).unwrap();
        assert_eq!(version_of(&connection), MIGRATIONS.len());
        assert_eq!(schema_of(&connection), schema_of(&fresh));
    }

    let newer_path = directory.join("newer.sqlite3");
    Connection::open(&newer_path)
        .unwrap()
        .pragma_update(None, "user_version", MIGRATIONS.len() + 1)
        .unwrap();
    assert!(AppStateStore::open(&newer_path).is_err());

    drop(fresh);
    std::fs::remove_dir_all(directory).unwrap();
}
