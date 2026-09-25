use super::*;

#[test]
#[ignore = "a merge keeps an object or map entry one side deleted when the other side edited it, instead of taking the delete and counting a conflict"]
fn a_schema_field_removed_on_one_side_and_renamed_on_the_other_stays_removed() {
    let (field, add) = DatabaseSchema::add_field("Name", DatabaseFieldType::String);
    let base = edited(&DatabaseSchemaContent::default(), [add]);
    let ours = edited(&base, [DatabaseSchema::remove_field(field)]);
    let theirs = edited(&base, [DatabaseSchema::rename_field(field, "Full name")]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);

    assert!(merged.root().fields().is_empty());
}
