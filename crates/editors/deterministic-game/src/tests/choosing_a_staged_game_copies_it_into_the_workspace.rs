use super::*;

use block_editor_beui::{BlockParent, DataListing, FetchResult, HostReply, HostRequest};
use block_ui_test::BeuiTest;

#[test]
fn choosing_a_staged_game_copies_it_into_the_workspace() {
    let creation = Creation::new(EditorHost::default());
    let mut editor = BeuiTest::<DeterministicGameApp>::creation(creation.clone());
    editor.run();

    let asked = editor.take_requests();
    let [(listing, HostRequest::ListData)] = asked.as_slice() else {
        panic!("the dialog asked for {asked:?} rather than the staged data");
    };
    editor.reply(
        *listing,
        HostReply::DataListed(DataListing::Files(vec![
            "games/tic_tac_toe.wasm".to_owned(),
            "notes.txt".to_owned(),
        ])),
    );
    editor.run();

    let asked = editor.take_requests();
    let [(read, HostRequest::ReadData(path))] = asked.as_slice() else {
        panic!("the dialog asked for {asked:?} rather than the one staged game");
    };
    assert_eq!(path, "games/tic_tac_toe.wasm");
    editor.reply(
        *read,
        HostReply::DataRead(FetchResult::Body(TIC_TAC_TOE.to_vec())),
    );
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
        BlockParent::Block(game)
    );
    let copied = store.content::<GameModuleContent>(Some(module));
    assert_eq!(copied.data(), TIC_TAC_TOE);
    assert_eq!(copied.header().source_name, "tic_tac_toe.wasm");
}
