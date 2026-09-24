use block::BlockParent;
use block_client::blocks::database::Database;
use block_client::blocks::database_view::DatabaseView;
use block_editor_plugin::be_block::database_view::{self, DatabaseViewContent};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::database::create_database;
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
        let database = create_database(creation);
        let view = creation
            .client()
            .create_block(DatabaseView::with_references(vec![database]));
        creation.seed_content(
            view.id(),
            &DatabaseViewContent::new(&database_view::DatabaseView::of(database)),
        );
        creation
            .client()
            .get_block::<Database>(database)
            .set_parent(BlockParent::Uuid(view.id()));
        Ok(view.id())
    }
}
