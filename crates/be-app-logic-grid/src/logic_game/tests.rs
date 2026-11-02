use be_sdk_beui::beui::Vec2;
use be_sdk_test::BeuiTest;
use be_type_logic_grid::LogicGame;
use be_type_logic_grid::LogicGameContent;
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
    let mut editor = BeuiTest::editing(block);
    if let Some(size) = size {
        editor = editor.with_size(size);
    }
    editor.store().register(be_type_logic_grid::registrations());
    editor.hold(None, LogicGameContent::default());
    editor.run();
    editor
}

fn game(editor: &BeuiTest<LogicGameApp>) -> LogicGame {
    editor.content::<LogicGameContent>(None).root().game()
}
