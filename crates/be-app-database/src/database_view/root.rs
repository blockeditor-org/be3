use be_sdk_beui::BlockParent;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_sdk_database::create_database;
use be_type_database::database_view::{self, DatabaseViewContent};
use uuid::Uuid;

pub(crate) mod calendar;
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
pub(crate) mod ui;

pub use data::value_block_filter;
use ui::{DatabaseViewEditor, DatabaseViewPreview};

pub struct DatabaseViewRoot;

#[component]
impl be_sdk_beui::BeuiRoot for DatabaseViewRoot {
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
