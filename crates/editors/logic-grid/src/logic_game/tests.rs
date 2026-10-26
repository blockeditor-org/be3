use block_editor_beui::beui::Vec2;
use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use logic_grid_types::LogicGame;
use logic_grid_types::LogicGameContent;
use logicgame::challenges::ChallengeId;
use uuid::Uuid;

use crate::logic_game::app::LogicGameApp;

mod a_manager_gives_the_world_a_hotbar_that_players_start_from;
mod expanding_a_level_shows_its_goal;
mod the_quiz_records_a_bit_on_the_game_block;

fn editor() -> BeuiTest<LogicGameApp> {
    editor_sized(None)
}

fn editor_sized(size: Option<Vec2>) -> BeuiTest<LogicGameApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host.clone(), block);
    let mut editor = match size {
        Some(size) => BeuiTest::new(editor).with_size(size),
        None => BeuiTest::new(editor),
    };
    editor.store().register(logic_grid_types::registrations());
    editor.hold(None, LogicGameContent::default());
    editor.run();
    editor
}

fn game(editor: &BeuiTest<LogicGameApp>) -> LogicGame {
    editor.content::<LogicGameContent>(None).root().game()
}
