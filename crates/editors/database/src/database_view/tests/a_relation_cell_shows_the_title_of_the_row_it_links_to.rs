use database_types::database::RowRef;
use database_types::database_schema::DatabaseRelationOptions;

use super::*;

#[test]
fn a_relation_cell_shows_the_title_of_the_row_it_links_to() {
    let mut fixture = editor(&[
        ("Title", DatabaseFieldType::String),
        ("Project", DatabaseFieldType::Relation),
    ]);
    let (title, project) = (fixture.fields[0], fixture.fields[1]);
    let (projects, projects_schema) = (Uuid::new_v4(), Uuid::new_v4());
    let mut schema = DatabaseSchemaContent::default();
    let (name, add_name) = DatabaseSchema::add_field("Name", DatabaseFieldType::String);
    schema.apply(&add_name);
    let (launch, add_launch) = Database::add_row([(name, DatabaseValue::String("Launch".into()))]);
    let mut listed = DatabaseContent::new(&Database::with_schema(projects_schema));
    listed.apply(&add_launch);
    fixture.harness.hold(Some(projects), listed);
    fixture.harness.hold(Some(projects_schema), schema);
    let schema_id = fixture.schema;
    fixture.harness.edit::<DatabaseSchemaContent>(
        Some(schema_id),
        &DatabaseSchema::set_relation_options(
            project,
            DatabaseRelationOptions {
                database: Some(projects),
                ..Default::default()
            },
        ),
    );
    fixture.set(0, title, DatabaseValue::String("Write the guide".into()));
    fixture.set(
        0,
        project,
        DatabaseValue::Relation(RowRef {
            database: projects,
            row: launch.as_uuid(),
        }),
    );

    let cell = format!("database-view.cell.0.{project}");
    fixture
        .harness
        .settle_until("the linked row's title", |harness| {
            harness.label(&cell) == "Launch"
        });
    fixture
        .harness
        .snapshot("a_relation_cell_shows_the_title_of_the_row_it_links_to");
}
