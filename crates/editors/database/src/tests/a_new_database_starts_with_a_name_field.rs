use super::*;
use block_editor_beui::be_block::database_schema::DatabaseSchemaContent;
use block_editor_beui::be_block::{BlockContent, Root};
use block_editor_beui::{BeuiApp, BlockParent, GraphCommand};

#[test]
fn a_new_database_starts_with_a_name_field() {
    let host = EditorHost::default();
    let creation = Creation::new(host.clone());

    let id = DatabaseApp::create_block(&creation).unwrap();

    let commands = host.take_graph_commands();
    let seeded_as = |block: Uuid| {
        commands
            .iter()
            .find_map(|command| match command {
                GraphCommand::Create {
                    id,
                    content: Some(content),
                    ..
                } if *id == block => Some(content.clone()),
                _ => None,
            })
            .expect("the block was given content")
    };
    let database = DatabaseContent::decode(&seeded_as(id)).unwrap().root();
    let schema_id = database.schema.unwrap();
    let schema = DatabaseSchemaContent::decode(&seeded_as(schema_id)).unwrap();
    let fields = schema.root().fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Name");

    assert_eq!(database.references(), [schema_id]);
    assert!(commands.contains(&GraphCommand::SetParent {
        id: schema_id,
        parent: BlockParent::Block(id),
    }));
}
