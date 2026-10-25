use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use database_types::DatabaseSchemaContent;
use database_types::database_schema::{DatabaseField, DatabaseFieldType};
use uuid::Uuid;

mod ui;

use ui::SchemaView;

pub struct DatabaseSchemaApp;

#[component]
impl block_editor_beui::BeuiApp for DatabaseSchemaApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <SchemaView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&DatabaseSchemaContent::default()))
    }
}

pub fn field_line_count(field: &DatabaseField) -> usize {
    1 + match field.field_type {
        DatabaseFieldType::Enum => field.enum_options.len() + 3,
        DatabaseFieldType::Number => 4,
        DatabaseFieldType::Block => 1,
        DatabaseFieldType::Relation => 2,
        DatabaseFieldType::String
        | DatabaseFieldType::Boolean
        | DatabaseFieldType::Color
        | DatabaseFieldType::Datetime => 0,
    }
}
