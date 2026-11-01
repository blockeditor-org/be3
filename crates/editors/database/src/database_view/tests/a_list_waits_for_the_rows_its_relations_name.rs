use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockParent};
use database_types::database::RowRef;
use database_types::database_schema::DatabaseRelationOptions;

use super::*;

#[test]
fn a_list_waits_for_the_rows_its_relations_name() {
    let (view, database, schema) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (projects, projects_schema) = (Uuid::new_v4(), Uuid::new_v4());

    let mut project_fields = DatabaseSchemaContent::default();
    let (name, add_name) = DatabaseSchema::add_field("Name", DatabaseFieldType::String);
    project_fields.apply(&add_name);
    let (launch, add_launch) = Database::add_row([(name, DatabaseValue::String("Launch".into()))]);
    let mut project_rows = DatabaseContent::new(&Database::with_schema(projects_schema));
    project_rows.apply(&add_launch);

    let mut fields = DatabaseSchemaContent::default();
    let (title, add_title) = DatabaseSchema::add_field("Title", DatabaseFieldType::String);
    fields.apply(&add_title);
    let (project, add_project) = DatabaseSchema::add_field("Project", DatabaseFieldType::Relation);
    fields.apply(&add_project);
    fields.apply(&DatabaseSchema::set_relation_options(
        project,
        DatabaseRelationOptions {
            database: Some(projects),
            ..Default::default()
        },
    ));
    let (row, add_row) = Database::add_row([
        (title, DatabaseValue::String("Write the guide".into())),
        (
            project,
            DatabaseValue::Relation(RowRef {
                database: projects,
                row: launch.as_uuid(),
            }),
        ),
    ]);
    let mut rows = DatabaseContent::new(&Database::with_schema(schema));
    rows.apply(&add_row);
    let mut shown = DatabaseViewContent::new(&DatabaseView::of(database));
    shown.apply(&DatabaseView::set_kind(DatabaseViewKind::List));

    let mut harness = BeuiTest::<DatabaseViewApp>::editing(view);
    harness.store().add_block(BlockInfo::new(
        projects,
        DatabaseContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    harness.store().add_block(BlockInfo::new(
        database,
        DatabaseContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    harness.store().add_block(BlockInfo::new(
        schema,
        DatabaseSchemaContent::CONTENT_TYPE,
        BlockParent::Block(database),
    ));
    harness.hold(None, shown);
    harness.hold(Some(database), rows);
    harness.hold(Some(schema), fields);
    harness.run();

    let line = format!("database-view.list.row.{}", row.as_uuid());
    assert!(harness.shown("editor.loading"));
    assert!(!harness.shown(&line));

    harness.hold(Some(projects), project_rows);
    harness.hold(Some(projects_schema), project_fields);
    harness.run();

    assert!(!harness.shown("editor.loading"));
    assert!(harness.label(&line).contains("Launch"));
}
