use block_editor_beui::Loaded;
use block_editor_beui::be_block::LogicGameContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod game;
mod ui;

use ui::LogicGameEditor;

pub(crate) type GameBlock =
    block_editor_beui::ContentProjection<block_editor_beui::be_block::LogicGameContent>;

pub(crate) fn operate(
    block: &GameBlock,
    operation: block_editor_beui::be_block::logic_game::LogicGameOperation,
) {
    if let Some(edit) = block.read(|game| game.root().edit_for(&operation)) {
        block.operate(edit);
    }
}

pub struct LogicGameApp;

impl block_editor_beui::BeuiApp for LogicGameApp {
    fn view(editor: Editor) -> NodeId {
        let loaded = editor.loaded::<block_editor_beui::be_block::LogicGameContent>(&[]);
        view! {
            <Loaded loaded={loaded}>
                <LogicGameEditor editor={editor} />
            </Loaded>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&LogicGameContent::default()))
    }
}
