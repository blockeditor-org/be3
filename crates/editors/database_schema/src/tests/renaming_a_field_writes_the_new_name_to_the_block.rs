use super::*;
use block_client::blocks::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchemaOperation,
};

#[test]
fn renaming_a_field_writes_the_new_name_to_the_block() {
    let (mut editor, block) = editor();
    let id = Uuid::new_v4();
    block.operate(DatabaseSchemaOperation::AddField {
        field: DatabaseField {
            id,
            name: "Field".into(),
            field_type: DatabaseFieldType::String,
            enum_options: Vec::new(),
            number_options: Default::default(),
            block_options: Default::default(),
        },
    });
    editor.run();

    editor.click(&format!("database-schema.name.{id}"));
    editor.run();
    editor.text("s");
    editor.run();

    assert_eq!(block.read().unwrap().fields()[0].name, "Fields");
}
