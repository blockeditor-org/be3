use super::*;
use database_types::database_schema::{ChoiceIcon, DatabaseFieldType};

#[test]
fn a_choice_option_takes_an_icon_and_can_be_closed_and_the_default() {
    let mut harness = editor();
    let (status, add_status) = DatabaseSchema::add_field("Status", DatabaseFieldType::Enum);
    let (backlog, add_backlog) = DatabaseSchema::add_enum_option(status, "Backlog");
    let (done, add_done) = DatabaseSchema::add_enum_option(status, "Done");
    for edit in [add_status, add_backlog, add_done] {
        harness.edit::<DatabaseSchemaContent>(None, &edit);
    }
    harness.run();

    harness.click(format!("database-schema.enum.{status}.{done}.icon"));
    harness.run();
    harness.click(format!("database-schema.enum.{status}.{done}.icon.done"));
    harness.run();
    harness.click(format!("database-schema.enum.{status}.{done}.closed"));
    harness.run();
    harness.click(format!("database-schema.enum.{status}.{backlog}.default"));
    harness.run();
    harness.click(format!("database-schema.enum.{status}.{done}.up"));
    harness.run();

    let field = schema(&harness).fields()[0].clone();
    let finished = field.option(done).unwrap();
    assert_eq!(finished.icon, ChoiceIcon::Done);
    assert!(finished.closed);
    assert_eq!(field.default_value(), Some(backlog));
    let order: Vec<_> = field.enum_options.iter().map(|option| option.id).collect();
    assert_eq!(order, [done, backlog]);
    harness.snapshot("a_choice_option_takes_an_icon_and_can_be_closed_and_the_default");
}
