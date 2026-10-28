use super::*;
use database_types::database_schema::DatabaseFieldType;

#[test]
fn renaming_a_field_writes_the_new_name_to_the_block() {
    let mut harness = editor();
    let (id, add) = DatabaseSchema::add_field("Field", DatabaseFieldType::String);
    harness.edit::<DatabaseSchemaContent>(None, &add);

    harness.click(format!("database-schema.name.{id}"));
    harness.type_text("s");

    assert_eq!(schema(&harness).fields()[0].name, "Fields");
}
