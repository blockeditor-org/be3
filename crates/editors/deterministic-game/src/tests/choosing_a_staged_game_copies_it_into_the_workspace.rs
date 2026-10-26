use super::*;

use block_editor_beui::BlockParent;
use block_ui_test::{BeuiTest, HostCall};

#[test]
fn choosing_a_staged_game_copies_it_into_the_workspace() {
    let creation = Creation::new(EditorHost::default());
    let mut editor = BeuiTest::<DeterministicGameApp>::creation(creation.clone());
    editor.run();

    let asked = editor.take_requests();
    let [HostCall::ListData { reply: listing }]: [_; 1] =
        asked.try_into().unwrap_or_else(|asked: Vec<_>| {
            panic!("the dialog asked for {asked:?} rather than the staged data")
        })
    else {
        panic!("the dialog asked for something other than the staged data");
    };
    editor.answer(
        listing,
        Ok(vec![
            "games/tic_tac_toe.wasm".to_owned(),
            "notes.txt".to_owned(),
        ]),
    );
    editor.run();

    let asked = editor.take_requests();
    let [HostCall::ReadData { reply: read, path }]: [_; 1] =
        asked.try_into().unwrap_or_else(|asked: Vec<_>| {
            panic!("the dialog asked for {asked:?} rather than the one staged game")
        })
    else {
        panic!("the dialog asked for something other than the one staged game");
    };
    assert_eq!(path, "games/tic_tac_toe.wasm");
    editor.answer(read, Ok(TIC_TAC_TOE.to_vec()));
    editor.run();

    let name = Game::load(TIC_TAC_TOE).unwrap().name().to_owned();
    assert_eq!(editor.label("game.staged.0"), name);
    editor.click("game.staged.0");
    editor.run();

    let game = creation.create_block().unwrap();
    editor.run();

    let store = editor.store();
    let module = store
        .content::<DeterministicGameContent>(Some(game))
        .root()
        .module
        .expect("the game plays a module");
    assert_ne!(module, game);
    assert_eq!(
        store.block(module).unwrap().parent,
        Some(BlockParent::Block(game))
    );
    let copied = store.content::<GameModuleContent>(Some(module));
    assert_eq!(copied.data(), TIC_TAC_TOE);
    assert_eq!(copied.header().source_name, "tic_tac_toe.wasm");
}
