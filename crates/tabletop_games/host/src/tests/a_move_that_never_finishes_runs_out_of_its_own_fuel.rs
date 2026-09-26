use uuid::Uuid;

use super::{Game, GameAction};

#[test]
fn a_move_that_never_finishes_runs_out_of_its_own_fuel() {
    let module = br#"
        (module
            (import "game" "next" (func $next (param i32 i32) (result i64)))
            (memory (export "memory") 1)
            (func (export "name") (result i64) (i64.const 0))
            (func (export "play")
                (drop (call $next (i32.const 0) (i32.const 1024)))
                (loop $forever (br $forever))))
    "#;
    let game = Game::load(module).expect("the module is shaped like a game");
    let mut session = game.start().expect("the game waits for its first move");
    let action = GameAction {
        actor: Uuid::new_v4(),
        action: Vec::new(),
    };

    let error = session.play(&action).expect_err("the move never finishes");

    assert_eq!(error, "this game ran out of fuel on move 1");
    assert_eq!(session.show(Uuid::new_v4()), Err(error));
    assert!(session.played().is_empty());
}
