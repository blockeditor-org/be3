use block_client::blocks::logic_game::LogicGame as LogicGameBlock;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod game;
mod ui;

use ui::LogicGameEditor;

pub(crate) type GameBlock = std::rc::Rc<
    block_editor_plugin::ContentProjection<block_editor_plugin::be_block::LogicGameContent>,
>;

pub(crate) fn operate(
    block: &GameBlock,
    operation: block_editor_plugin::be_block::logic_game::LogicGameOperation,
) {
    if let Some(edit) = block.read(|game| game.root().edit_for(&operation)) {
        block.operate(edit);
    }
}

pub struct LogicGameApp;

impl block_editor_plugin::BeuiApp for LogicGameApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <LogicGameEditor editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(LogicGameBlock::new()).id())
    }
}
