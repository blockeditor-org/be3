use block_editor_beui::BlockParent;
use block_editor_beui::be_block::database_view::{self, DatabaseViewContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::database::create_database;
use block_editor_beui::{Creation, Editor};
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

impl block_editor_beui::BeuiApp for DatabaseViewApp {
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
        let view = creation.create(&DatabaseViewContent::new(&database_view::DatabaseView::of(
            database,
        )));
        creation
            .blocks()
            .set_parent(database, BlockParent::Block(view));
        Ok(view)
    }
}
