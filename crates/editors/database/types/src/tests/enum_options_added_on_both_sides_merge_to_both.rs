use super::*;

#[test]
fn enum_options_added_on_both_sides_merge_to_both() {
    let (field, add) = DatabaseSchema::add_field("Status", DatabaseFieldType::Enum);
    let (todo, add_todo) = DatabaseSchema::add_enum_option(field, "Todo");
    let base = edited(&DatabaseSchemaContent::default(), [add, add_todo]);
    let (doing, add_doing) = DatabaseSchema::add_enum_option(field, "Doing");
    let ours = edited(&base, [add_doing]);
    let (done, add_done) = DatabaseSchema::add_enum_option(field, "Done");
    let theirs = edited(
        &base,
        [add_done, DatabaseSchema::rename_enum_option(todo, "To do")],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    let options: Vec<(Uuid, String)> = merged.root().fields()[0]
        .enum_options
        .iter()
        .map(|option| (option.id, option.name.clone()))
        .collect();
    assert_eq!(
        options,
        [
            (todo, "To do".to_owned()),
            (doing, "Doing".to_owned()),
            (done, "Done".to_owned())
        ]
    );
}
