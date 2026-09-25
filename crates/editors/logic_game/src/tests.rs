use block_editor_plugin::be_block::LogicGameContent;
use block_editor_plugin::be_block::logic_game::LogicGame;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use logicgame::challenges::ChallengeId;
use uuid::Uuid;

use crate::app::LogicGameApp;

mod expanding_a_level_shows_its_goal;
mod the_quiz_records_a_bit_on_the_game_block;

fn editor() -> BeuiTest<LogicGameApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, LogicGameContent::default());
    editor.run();
    editor.run();
    editor
}

fn game(editor: &BeuiTest<LogicGameApp>) -> LogicGame {
    editor.content::<LogicGameContent>(None).root().game()
}
