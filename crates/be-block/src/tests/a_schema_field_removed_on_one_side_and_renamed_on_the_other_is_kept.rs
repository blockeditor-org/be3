use super::*;

#[test]
fn a_schema_field_removed_on_one_side_and_renamed_on_the_other_is_kept() {
    let (field, add) = DatabaseSchema::add_field("Name", DatabaseFieldType::String);
    let base = edited(&DatabaseSchemaContent::default(), [add]);
    let ours = edited(&base, [DatabaseSchema::remove_field(field)]);
    let theirs = edited(&base, [DatabaseSchema::rename_field(field, "Full name")]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    let fields = merged.root().fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].id, field);
    assert_eq!(fields[0].name, "Full name");
}
