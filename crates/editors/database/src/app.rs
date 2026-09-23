use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::database::create_database;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::DatabaseEditor;

pub struct DatabaseApp;

impl block_editor_plugin::BeuiApp for DatabaseApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <DatabaseEditor editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(create_database(creation))
    }
}
