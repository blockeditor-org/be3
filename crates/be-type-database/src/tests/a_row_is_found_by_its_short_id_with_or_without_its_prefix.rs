use super::*;

#[test]
fn a_row_is_found_by_its_short_id_with_or_without_its_prefix() {
    let title = Uuid::new_v4();
    let (first, add_first) = Database::add_row([(title, DatabaseValue::String("One".into()))]);
    let (second, add_second) = Database::add_row([(title, DatabaseValue::String("Two".into()))]);
    let content = edited(
        &DatabaseContent::default(),
        [add_first, add_second, Database::set_prefix(" BE ")],
    );
    let database = content.root();

    let code = database.short_id(second);
    assert!(code.starts_with("BE-"), "{code}");
    assert_eq!(code.len(), "BE-".len() + 6);
    assert_ne!(code, database.short_id(first));
    assert_eq!(database.find_short_id(&code), Some(second));
    assert_eq!(
        database.find_short_id(&code[3..].to_lowercase()),
        Some(second)
    );
    assert_eq!(database.find_short_id("BE-"), None);
    assert_eq!(database.title(second, Some(title)), "Two");
    assert_eq!(database.title(first, None), database.short_id(first));
}
