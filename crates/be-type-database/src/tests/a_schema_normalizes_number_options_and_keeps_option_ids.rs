use super::*;

#[test]
fn a_schema_normalizes_number_options_and_keeps_option_ids() {
    let (count, add_count) = DatabaseSchema::add_field("Count", DatabaseFieldType::Number);
    let (status, add_status) = DatabaseSchema::add_field("Status", DatabaseFieldType::Enum);
    let (open, add_open) = DatabaseSchema::add_enum_option(status, "Open");
    let schema = edited(
        &DatabaseSchemaContent::default(),
        [
            add_count,
            add_status,
            add_open,
            DatabaseSchema::set_number_options(
                count,
                DatabaseNumberOptions {
                    minimum: Some(20.0),
                    maximum: Some(10.0),
                    step: Some(-1.0),
                    scale: DatabaseNumberScale::Linear,
                },
            ),
            DatabaseSchema::rename_enum_option(open, "Opened"),
            DatabaseSchema::rename_field(status, "State"),
        ],
    );

    let fields = schema.root().fields();
    assert_eq!(fields[0].id, count);
    assert_eq!(
        fields[0].number_options,
        DatabaseNumberOptions {
            minimum: Some(10.0),
            maximum: Some(20.0),
            step: None,
            scale: DatabaseNumberScale::Linear,
        }
    );
    assert_eq!(fields[1].id, status);
    assert_eq!(fields[1].name, "State");
    assert_eq!(fields[1].enum_options[0].id, open);
    assert_eq!(fields[1].enum_options[0].name, "Opened");

    let schema = edited(&schema, [DatabaseSchema::remove_enum_option(open)]);
    assert!(schema.root().fields()[1].enum_options.is_empty());
}
