use super::*;
use crate::database::{Database, DatabaseContent};

#[test]
fn a_database_is_described_by_its_prefix() {
    let mut database = Database::with_schema(Uuid::from_u128(7));
    database.prefix = " BE ".to_owned();
    let content = DatabaseContent::new(&database);

    let described = BlockMetadata {
        derived: derived_metadata(&content),
        ..BlockMetadata::default()
    };
    assert_eq!(described.get(&metadata::ROW_PREFIX), Some("BE".to_owned()));
    assert_eq!(content.name(), None);
}
