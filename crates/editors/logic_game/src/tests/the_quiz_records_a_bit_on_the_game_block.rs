use super::*;

#[test]
fn the_quiz_records_a_bit_on_the_game_block() {
    let mut editor = editor();

    let level = ChallengeId::BinaryAddition;
    editor.click(&format!("logic-game.level.{}", level as usize));
    editor.run();
    editor.click("quiz.sum.0");
    editor.run();

    assert_eq!(
        game(&editor).quiz(0).map(|answers| answers.sums[0]),
        Some(Some(false))
    );
}
