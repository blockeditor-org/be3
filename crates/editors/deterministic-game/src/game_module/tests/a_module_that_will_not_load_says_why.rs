use super::*;

#[test]
fn a_module_that_will_not_load_says_why() {
    let (mut test, _editor) = editor(b"not a wasm module".to_vec());

    assert!(test.shown("game-module.error"));
    test.snapshot("a_module_that_will_not_load_says_why");
}
