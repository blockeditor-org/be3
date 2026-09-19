use block_client::blocks::logic_game::LogicGame;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod game;
mod ui;

use ui::LogicGameEditor;

pub struct LogicGameApp;

impl block_editor_plugin::BeuiApp for LogicGameApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <LogicGameEditor editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(LogicGame::new()).id())
    }
}
