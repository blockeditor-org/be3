use block_editor_beui::be_block::BlockContent;

use block_editor_beui::be_block::{DeterministicGame, DeterministicGameContent, GameModuleContent};
use block_editor_beui::beui::{Pos2, Vec2};
use block_editor_beui::{Creation, Editor, EditorHost};
use block_ui_test::BeuiTest;
use game_api::{GameAction, GameActionOption};
use game_host::Game;
use uuid::Uuid;

use crate::app::{DeterministicGameApp, module_filter, seats};

mod a_module_that_is_not_a_game_is_reported;
mod a_new_player_can_take_the_other_side;
mod a_piece_is_dragged_from_square_to_square;
mod a_promotion_asks_which_piece_to_become;
mod clicking_a_card_then_the_discard_pile_plays_it;
mod clicking_an_open_tile_places_a_mark;
mod clicking_the_draw_pile_draws_a_card;
mod dragging_a_card_onto_the_discard_pile_plays_it;
mod dropping_an_eight_asks_which_suit_to_call;
mod holding_a_square_and_drawing_with_another_finger_draws_an_arrow;
mod only_you_and_the_guests_you_brought_can_be_played_as;
mod right_dragging_draws_an_arrow_and_right_clicking_circles_a_square;
mod the_board_is_drawn_through_the_view_the_host_gives;
mod the_creation_dialog_is_drawn_with_beui;
mod the_history_lists_every_move_and_steps_back_through_it;
mod the_picker_asks_only_for_game_modules;
mod without_the_chrome_the_status_and_buttons_sit_under_the_board;

const ACCOUNT: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d61_6363_6f75_6e74);
const OPPONENT: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d6f_7070_6f6e_656e);
const TIC_TAC_TOE: &[u8] = include_bytes!(env!("TIC_TAC_TOE_WASM"));
const CHESS: &[u8] = include_bytes!(env!("CHESS_WASM"));
const CRAZY_8S: &[u8] = include_bytes!(env!("CRAZY_8S_WASM"));

fn editor(module: Vec<u8>) -> BeuiTest<DeterministicGameApp> {
    editor_after(module, Vec::new())
}

fn editor_after(module: Vec<u8>, moves: Vec<GameAction>) -> BeuiTest<DeterministicGameApp> {
    placed(module, moves, true)
}

fn placed(
    module: Vec<u8>,
    moves: Vec<GameAction>,
    in_viewport: bool,
) -> BeuiTest<DeterministicGameApp> {
    let module_block = Uuid::new_v4();
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_account_id(ACCOUNT);
    let mut harness = BeuiTest::new(Editor::new(host.clone(), block));
    if in_viewport {
        harness = harness.in_viewport();
    }
    harness.hold(
        None,
        DeterministicGameContent::new(&DeterministicGame::of(module_block)),
    );
    for played in moves {
        harness.edit::<DeterministicGameContent>(
            None,
            &DeterministicGame::play(played.actor, played.action),
        );
    }
    harness.hold(
        Some(module_block),
        GameModuleContent::from_file("game.wasm", module),
    );
    harness.run();
    harness.run();
    harness
}

fn creation_editor() -> BeuiTest<DeterministicGameApp> {
    let host = EditorHost::default();
    BeuiTest::creation(Creation::new(host))
}

fn moves(harness: &BeuiTest<DeterministicGameApp>) -> Vec<GameAction> {
    harness
        .content::<DeterministicGameContent>(None)
        .root()
        .moves
        .iter()
        .map(|played| GameAction {
            actor: played.actor,
            action: played.action.clone(),
        })
        .collect()
}

fn offered(module: &[u8], actions: &[GameAction], player: Uuid) -> Vec<GameActionOption> {
    Game::load(module)
        .expect("the test module is a game")
        .show(actions, player)
        .expect("the game answers")
        .actions
}

fn taken(module: &[u8], actions: &mut Vec<GameAction>, actor: Uuid, label: &str) {
    let option = offered(module, actions, actor)
        .into_iter()
        .find(|option| option.label == label)
        .unwrap_or_else(|| panic!("{label} is not offered"));
    actions.push(GameAction {
        actor,
        action: option.effect,
    });
}

fn dealt() -> Vec<GameAction> {
    dealt_against(OPPONENT)
}

fn dealt_against(opponent: Uuid) -> Vec<GameAction> {
    let mut actions = Vec::new();
    taken(CRAZY_8S, &mut actions, ACCOUNT, "Join the game");
    taken(CRAZY_8S, &mut actions, opponent, "Join the game");
    taken(CRAZY_8S, &mut actions, ACCOUNT, "Start the game");
    actions
}

fn a_plain_play(actions: &[GameAction]) -> GameActionOption {
    offered(CRAZY_8S, actions, ACCOUNT)
        .into_iter()
        .find(|option| option.label.starts_with("Play") && !option.label.contains("call"))
        .expect("the deal leaves a card that plays without calling a suit")
}

fn played(module: &[u8], moves: &[(Uuid, &str)]) -> Vec<GameAction> {
    let mut actions = Vec::new();
    for (actor, label) in moves {
        taken(module, &mut actions, *actor, label);
    }
    actions
}

fn on_the_card(harness: &BeuiTest<DeterministicGameApp>, test_id: &str) -> Pos2 {
    harness.rect_of(test_id).min + Vec2::new(8.0, 40.0)
}
