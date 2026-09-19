use std::sync::Arc;

use block_client::blocks::logic_game::LogicGame;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use logicgame::challenges::ChallengeId;
use uuid::Uuid;

use crate::app::LogicGameApp;

mod expanding_a_level_shows_its_goal;
mod the_quiz_records_a_bit_on_the_game_block;

fn editor() -> (BeuiTest<LogicGameApp>, BlockHandle<LogicGame>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(LogicGame::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    editor.run();
    (editor, block)
}
