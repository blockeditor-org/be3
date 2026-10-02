use rusqlite::Connection;

use super::*;
use crate::schema::{MIGRATIONS, initialize, migrate};

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
fn every_older_database_schema_migrates_to_the_current_one() {
    let fresh = Connection::open_in_memory().unwrap();
    initialize(&fresh).unwrap();
    assert_eq!(version_of(&fresh), MIGRATIONS.len());

    for older in 0..MIGRATIONS.len() {
        let connection = Connection::open_in_memory().unwrap();
        migrate(&connection, &MIGRATIONS[..older]).unwrap();
        assert_eq!(version_of(&connection), older);
        initialize(&connection).unwrap();
        assert_eq!(version_of(&connection), MIGRATIONS.len());
        assert_eq!(schema_of(&connection), schema_of(&fresh));
    }

    let unversioned = Connection::open_in_memory().unwrap();
    unversioned.execute_batch(MIGRATIONS[0]).unwrap();
    initialize(&unversioned).unwrap();
    assert_eq!(schema_of(&unversioned), schema_of(&fresh));

    let newer = Connection::open_in_memory().unwrap();
    newer
        .pragma_update(None, "user_version", MIGRATIONS.len() + 1)
        .unwrap();
    assert!(matches!(
        initialize(&newer),
        Err(ServerError::Refused(ErrorCode::UpdateRequired, _))
    ));
}
