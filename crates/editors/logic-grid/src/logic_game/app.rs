use block_editor_beui::Loaded;
use block_editor_beui::be_block::LogicGameContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
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
    if let Ok(edit) = block.read(|game| game.root().edit_for(&operation)) {
        block.operate(edit);
    }
}

pub struct LogicGameApp;

impl block_editor_beui::BeuiApp for LogicGameApp {
    fn view(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <LogicGameEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&LogicGameContent::default()))
    }
}
