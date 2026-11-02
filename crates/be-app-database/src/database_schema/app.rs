use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_database::DatabaseSchemaContent;
use be_type_database::database_schema::{DatabaseField, DatabaseFieldType};
use uuid::Uuid;

mod ui;

use ui::SchemaView;

pub struct DatabaseSchemaApp;

#[component]
impl be_sdk_beui::BeuiApp for DatabaseSchemaApp {
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
