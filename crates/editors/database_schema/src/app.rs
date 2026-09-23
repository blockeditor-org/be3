use block_client::blocks::database_schema::DatabaseSchema;
use block_editor_plugin::be_block::database_schema::{DatabaseField, DatabaseFieldType};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::SchemaView;

pub struct DatabaseSchemaApp;

impl block_editor_plugin::BeuiApp for DatabaseSchemaApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <SchemaView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(DatabaseSchema::new()).id())
    }
}

pub fn field_line_count(field: &DatabaseField) -> usize {
    1 + match field.field_type {
        DatabaseFieldType::Enum => field.enum_options.len() + 1,
        DatabaseFieldType::Number => 4,
        DatabaseFieldType::Block => 1,
        DatabaseFieldType::String
        | DatabaseFieldType::Boolean
        | DatabaseFieldType::Color
        | DatabaseFieldType::Datetime => 0,
    }
}
