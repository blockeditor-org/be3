use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_logic_grid::LogicGameContent;
use uuid::Uuid;

mod game;
mod ui;

use ui::LogicGameEditor;

pub(crate) type GameBlock = be_sdk_beui::ContentProjection<be_type_logic_grid::LogicGameContent>;

pub(crate) fn operate(block: &GameBlock, operation: be_type_logic_grid::LogicGameOperation) {
    if let Ok(edit) = block.read(|game| game.root().edit_for(&operation)) {
        block.operate(edit);
    }
}

pub struct LogicGameApp;

#[component]
impl be_sdk_beui::BeuiApp for LogicGameApp {
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
