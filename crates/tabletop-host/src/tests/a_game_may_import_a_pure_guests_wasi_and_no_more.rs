use super::Game;

fn game(imports: &str) -> String {
    format!(
        r#"(module
            {imports}
            (memory (export "memory") 1)
            (data (i32.const 16) "dice")
            (func (export "name") (result i64)
                (drop (call $random (i32.const 0) (i32.const 8)))
                (i64.or (i64.shl (i64.const 16) (i64.const 32)) (i64.const 4))))"#
    )
}

#[test]
fn a_game_may_import_a_pure_guests_wasi_and_no_more() {
    let random = r#"(import "wasi_snapshot_preview1" "random_get" (func $random (param i32 i32) (result i32)))
        (import "wasi_snapshot_preview1" "proc_exit" (func (param i32)))"#;
    let loaded = Game::load(game(random).as_bytes()).expect("a game may draw randomness");
    assert_eq!(loaded.name(), "dice");
    let reads = format!(
        r#"{random} (import "wasi_snapshot_preview1" "fd_read" (func (param i32 i32 i32 i32) (result i32)))"#
    );
    let Err(refused) = Game::load(game(&reads).as_bytes()) else {
        panic!("a game that reads a file loaded");
    };
    assert!(refused.contains("fd_read"), "{refused}");
}
