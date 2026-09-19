use super::*;

#[test]
fn expanding_a_level_shows_its_goal() {
    let (mut editor, block) = editor();

    let first = block.read().unwrap().levels()[0].challenge;
    editor.click(&format!("logic-game.level.{}", first as usize));
    editor.run();

    assert!(editor.shown(&format!("logic-game.new-attempt.{}", first as usize)));
    editor.snapshot("expanding_a_level_shows_its_goal");
}
