use super::*;
use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockParent};
use database_types::database::DatabaseContent;
use database_types::database_schema::{DatabaseFieldType, DatabaseRelationOptions};

#[test]
fn a_relation_field_names_the_database_it_links_to() {
    let mut harness = editor();
    let projects = Uuid::new_v4();
    let mut info = BlockInfo::new(projects, DatabaseContent::CONTENT_TYPE, BlockParent::Root);
    info.set_name("Projects");
    harness.store().add_block(info);
    let (project, add_project) = DatabaseSchema::add_field("Project", DatabaseFieldType::Relation);
    for edit in [
        add_project,
        DatabaseSchema::set_relation_options(
            project,
            DatabaseRelationOptions {
                database: Some(projects),
                ..Default::default()
            },
        ),
    ] {
        harness.edit::<DatabaseSchemaContent>(None, &edit);
    }
    let button = format!("database-schema.relation.{project}.database");
    harness.settle_until("the linked database's name", |harness| {
        harness.label(&button) == "Projects"
    });
    assert_eq!(
        schema(&harness).fields()[0].relation_options.database,
        Some(projects)
    );
    harness.snapshot("a_relation_field_names_the_database_it_links_to");
}
