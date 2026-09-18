use block::BlockParent;
use block_client::block_ref::BlockRef;
use block_client::blocks::database::Database;
use block_client::blocks::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::DatabaseEditor;

pub struct DatabaseApp;

impl block_editor_plugin::BeuiApp for DatabaseApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <DatabaseEditor editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        let client = creation.client();
        let schema = client.create_block(DatabaseSchema::new());
        schema.operate(DatabaseSchemaOperation::AddField {
            field: DatabaseField {
                id: Uuid::new_v4(),
                name: "Name".into(),
                field_type: DatabaseFieldType::String,
                enum_options: Vec::new(),
                number_options: Default::default(),
                block_options: Default::default(),
            },
        });
        let database = client.create_block(Database::new(BlockRef::Direct(schema.id())));
        schema.set_parent(BlockParent::Uuid(database.id()));
        Ok(database.id())
    }
}
