use block_editor_beui::be_block::DatabaseSchemaContent;
use block_editor_beui::be_block::database_schema::{DatabaseField, DatabaseFieldType};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::SchemaView;

pub struct DatabaseSchemaApp;

impl block_editor_beui::BeuiApp for DatabaseSchemaApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <SchemaView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&DatabaseSchemaContent::default()))
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
