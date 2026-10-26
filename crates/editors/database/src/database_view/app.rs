use block_editor_beui::BlockParent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::database::create_database;
use block_editor_beui::{Creation, Editor};
use database_types::database_view::{self, DatabaseViewContent};
use uuid::Uuid;

pub(crate) mod controls;
pub(crate) mod data;
pub(crate) mod display;
pub(crate) mod filters;
pub(crate) mod header;
pub(crate) mod kanban;
pub(crate) mod list;
pub(crate) mod row;
pub(crate) mod scatter;
pub(crate) mod spreadsheet;
mod ui;

pub use data::value_block_filter;
use ui::{DatabaseViewEditor, DatabaseViewPreview};

pub struct DatabaseViewApp;

#[component]
impl block_editor_beui::BeuiApp for DatabaseViewApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <DatabaseViewEditor editor={editor} />
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
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
