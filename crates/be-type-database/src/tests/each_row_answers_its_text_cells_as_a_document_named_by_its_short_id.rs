use std::collections::HashSet;

use be_block::{PartReader, SearchDocument};

use super::*;

#[test]
fn each_row_answers_its_text_cells_as_a_document_named_by_its_short_id() {
    let (title, notes, estimate) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let (first, add_first) = Database::add_row([
        (title, DatabaseValue::String("Fix the login".into())),
        (notes, DatabaseValue::String("Fails offline".into())),
        (estimate, DatabaseValue::Number(3.0)),
    ]);
    let (second, add_second) = Database::add_row([(estimate, DatabaseValue::Number(1.0))]);
    let content = edited(
        &DatabaseContent::default(),
        [add_first, add_second, Database::set_prefix("BE")],
    );
    let parts = PartReader::new(|_, _, _| panic!("a database is loaded whole and reads no part"));
    let database = content.root();
    assert_eq!(
        pollster::block_on(content.search_text(None, &HashSet::new(), parts)),
        Ok(vec![
            SearchDocument {
                key: first.as_uuid().to_string(),
                source: None,
                name: Some(database.short_id(first)),
                body: Some("Fix the login\nFails offline".to_owned()),
            },
            SearchDocument {
                key: second.as_uuid().to_string(),
                source: None,
                name: Some(database.short_id(second)),
                body: Some(String::new()),
            },
        ])
    );
}
