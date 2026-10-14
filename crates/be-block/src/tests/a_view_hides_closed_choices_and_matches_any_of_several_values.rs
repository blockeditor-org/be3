use super::*;
use crate::database_schema::{ChoiceColor, ChoiceIcon};
use crate::database_view::FilterCondition;

#[test]
fn a_view_hides_closed_choices_and_matches_any_of_several_values() {
    let (title, _) = DatabaseSchema::add_field("Title", DatabaseFieldType::String);
    let (status, add_status) = DatabaseSchema::add_field("Status", DatabaseFieldType::Enum);
    let (backlog, add_backlog) = DatabaseSchema::add_enum_option(status, "Backlog");
    let (review, add_review) = DatabaseSchema::add_enum_option(status, "In Review");
    let (done, add_done) = DatabaseSchema::add_enum_option(status, "Done");
    let schema = edited(
        &DatabaseSchemaContent::default(),
        [
            add_status,
            add_backlog,
            add_review,
            add_done,
            DatabaseSchema::set_option_closed(done, true),
            DatabaseSchema::set_option_icon(done, ChoiceIcon::Done),
            DatabaseSchema::set_option_color(done, ChoiceColor::Blue),
            DatabaseSchema::set_default_option(status, Some(backlog)),
            DatabaseSchema::move_enum_option(status, review, None),
        ],
    );
    let fields = schema.root().fields();
    let field = &fields[0];
    let order: Vec<_> = field.enum_options.iter().map(|option| option.id).collect();
    assert_eq!(order, [review, backlog, done]);
    let finished = field.option(done).unwrap();
    assert_eq!(
        (finished.closed, finished.icon, finished.color),
        (true, ChoiceIcon::Done, ChoiceColor::Blue)
    );
    assert_eq!(field.first_closed().map(|option| option.id), Some(done));
    assert_eq!(
        schema.root().defaults(),
        [(status, DatabaseValue::Enum(backlog))]
    );

    let database = edited(
        &DatabaseContent::default(),
        [("Plan", backlog), ("Review it", review), ("Ship", done)].map(|(name, state)| {
            Database::add_row([
                (title, DatabaseValue::String(name.into())),
                (status, DatabaseValue::Enum(state)),
            ])
            .1
        }),
    );
    let shown = |condition: FilterCondition| {
        let (_, filter) = DatabaseView::add_filter(Some(status), condition);
        let view = edited(&DatabaseViewContent::default(), [filter]).root();
        let database = database.root();
        database
            .rows
            .iter()
            .filter(|row| view.shows(row, &fields))
            .map(|row| database.title(row.id, Some(title)))
            .collect::<Vec<_>>()
    };
    assert_eq!(shown(FilterCondition::IsOpen), ["Plan", "Review it"]);
    assert_eq!(shown(FilterCondition::IsClosed), ["Ship"]);
    assert_eq!(
        shown(FilterCondition::AnyOf(vec![
            DatabaseValue::Enum(backlog),
            DatabaseValue::Enum(done)
        ])),
        ["Plan", "Ship"]
    );
    assert_eq!(
        shown(FilterCondition::NoneOf(vec![DatabaseValue::Enum(done)])),
        ["Plan", "Review it"]
    );
}
