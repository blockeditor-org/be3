use super::*;
use block_editor_plugin::be_block::database_schema::{
    DatabaseBlockOptions, DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale,
};

#[test]
fn configured_fields_paint_on_separate_lines() {
    let mut harness = editor();
    let (status, add_status) = DatabaseSchema::add_field("Status", DatabaseFieldType::Enum);
    let (_, add_ready) = DatabaseSchema::add_enum_option(status, "Ready");
    let (_, add_blocked) = DatabaseSchema::add_enum_option(status, "Blocked");
    let (estimate, add_estimate) = DatabaseSchema::add_field("Estimate", DatabaseFieldType::Number);
    let (attachment, add_attachment) =
        DatabaseSchema::add_field("Attachment", DatabaseFieldType::Block);
    for edit in [
        add_status,
        add_ready,
        add_blocked,
        add_estimate,
        DatabaseSchema::set_number_options(
            estimate,
            DatabaseNumberOptions {
                minimum: Some(1.0),
                maximum: Some(100.0),
                step: Some(1.1),
                scale: DatabaseNumberScale::Logarithmic,
            },
        ),
        add_attachment,
        DatabaseSchema::set_block_options(
            attachment,
            DatabaseBlockOptions {
                block_type: Some(Uuid::from_u128(7)),
            },
        ),
    ] {
        harness.edit::<DatabaseSchemaContent>(None, &edit);
    }
    harness.run();
    harness.snapshot("configured_fields_paint_on_separate_lines");
}
