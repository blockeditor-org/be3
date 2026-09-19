use block::BlockParent;
use block_client::block_ref::BlockRef;
use block_client::blocks::database::Database;
use block_client::blocks::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
};
use block_client::blocks::database_view::DatabaseView;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

pub(crate) mod data;
pub(crate) mod kanban;
pub(crate) mod scatter;
pub(crate) mod sidebar;
pub(crate) mod spreadsheet;
mod ui;

pub use data::value_block_filter;
use ui::{DatabaseViewEditor, DatabaseViewPreview};

pub struct DatabaseViewApp;

impl block_editor_plugin::BeuiApp for DatabaseViewApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <DatabaseViewEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <DatabaseViewPreview editor={editor} />
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
        let view = client.create_block(DatabaseView::new(BlockRef::Direct(database.id())));
        database.set_parent(BlockParent::Uuid(view.id()));
        Ok(view.id())
    }
}
