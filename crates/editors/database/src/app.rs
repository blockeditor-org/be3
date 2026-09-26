use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::database::create_database;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::DatabaseEditor;

pub struct DatabaseApp;

impl block_editor_beui::BeuiApp for DatabaseApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <DatabaseEditor editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(create_database(creation))
    }
}
