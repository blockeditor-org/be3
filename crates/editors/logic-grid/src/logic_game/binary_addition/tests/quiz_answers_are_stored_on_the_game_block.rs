use block_editor_beui::be_block::LogicGameContent;
use block_editor_beui::be_block::logic_game::LogicGameOperation;

use super::*;

#[test]
fn quiz_answers_are_stored_on_the_game_block() {
    let mut game = LogicGameContent::default();
    let quiz = BinaryAdditionQuiz::default();

    let (carries, sums) = stored(&quiz, &game, 0);
    assert_eq!(carries, vec![None; 5]);
    assert_eq!(sums, vec![None; 6]);
    assert!(!quiz.is_correct(&carries, &sums, 0));

    for (row, values) in [
        (QuizRow::Carries, quiz.problems()[0].carry_bits()),
        (QuizRow::Sums, quiz.problems()[0].sum_bits()),
    ] {
        let edit = game.root().edit_for(&LogicGameOperation::SetQuizRow {
            problem: 0,
            row,
            values: values.iter().copied().map(Some).collect(),
        });
        game.apply(&edit);
    }

    let (carries, sums) = stored(&quiz, &game, 0);
    assert!(quiz.is_correct(&carries, &sums, 0));

    let (carries, sums) = stored(&quiz, &game, 1);
    assert!(!quiz.is_correct(&carries, &sums, 1));
}

fn stored(
    quiz: &BinaryAdditionQuiz,
    game: &LogicGameContent,
    problem: usize,
) -> (Vec<Option<bool>>, Vec<Option<bool>>) {
    let (carries, sums) = game
        .root()
        .game()
        .quiz(problem)
        .map(|answers| (answers.carries.clone(), answers.sums.clone()))
        .unwrap_or_default();
    quiz.fit(problem, carries, sums)
}
