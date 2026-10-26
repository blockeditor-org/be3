use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use logic_grid_types::LogicGameContent;
use uuid::Uuid;

mod game;
mod ui;

use ui::LogicGameEditor;

pub(crate) type GameBlock =
    block_editor_beui::ContentProjection<logic_grid_types::LogicGameContent>;

pub(crate) fn operate(block: &GameBlock, operation: logic_grid_types::LogicGameOperation) {
    if let Ok(edit) = block.read(|game| game.root().edit_for(&operation)) {
        block.operate(edit);
    }
}

pub struct LogicGameApp;

#[component]
impl block_editor_beui::BeuiApp for LogicGameApp {
    #[component]
    fn View(editor: Editor) -> Root {
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
