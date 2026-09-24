use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::logic_game::LogicGame as GameBlock;
use block_editor_plugin::be_block::LogicGameContent;
use block_editor_plugin::be_block::logic_game::LogicGame;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use logicgame::challenges::ChallengeId;
use uuid::Uuid;

use crate::app::LogicGameApp;

mod expanding_a_level_shows_its_goal;
mod the_quiz_records_a_bit_on_the_game_block;

fn editor() -> ContentHarness<LogicGameApp> {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(GameBlock::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, block.id());
    let mut editor = ContentHarness::new(BeuiTest::new(editor), host);
    editor.hold(None, LogicGameContent::default());
    editor.run();
    editor.run();
    editor
}

fn game(editor: &ContentHarness<LogicGameApp>) -> LogicGame {
    editor.content::<LogicGameContent>(None).root().game()
}
