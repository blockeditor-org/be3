use super::*;
use block::Block;
use block_editor_plugin::BeuiApp;
use block_editor_plugin::be_block::BlockContent;
use block_editor_plugin::be_block::database_schema::DatabaseSchemaContent;

#[test]
fn a_new_database_starts_with_a_name_field() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let host = EditorHost::default();
    let creation = Creation::new(host.clone(), Arc::clone(&client));

    let id = DatabaseApp::create_block(&creation).unwrap();

    let seeded = host.take_seeded_content();
    let seeded_as = |block: Uuid| {
        seeded
            .iter()
            .find(|seeded| seeded.block == block)
            .map(|seeded| seeded.bytes.clone())
            .expect("the block was given content")
    };
    let database = DatabaseContent::decode(&seeded_as(id)).unwrap().root();
    let schema_id = database.schema.and_then(|schema| Some(schema)).unwrap();
    let schema = DatabaseSchemaContent::decode(&seeded_as(schema_id)).unwrap();
    let fields = schema.root().fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Name");

    let block = client.get_block::<DatabaseBlock>(id);
    assert_eq!(block.read().unwrap().references(), [schema_id]);
}
