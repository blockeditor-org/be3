use block_client::BlockClient;
use uuid::Uuid;

use super::*;

#[test]
fn quiz_answers_are_stored_on_the_game_block() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(LogicGame::new());
    let quiz = BinaryAdditionQuiz::default();

    let (carries, sums) = stored(&quiz, &block, 0);
    assert_eq!(carries, vec![None; 5]);
    assert_eq!(sums, vec![None; 6]);
    assert!(!quiz.is_correct(&carries, &sums, 0));

    quiz.write_row(
        &block,
        0,
        QuizRow::Carries,
        quiz.problems()[0]
            .carry_bits()
            .iter()
            .copied()
            .map(Some)
            .collect(),
    );
    quiz.write_row(
        &block,
        0,
        QuizRow::Sums,
        quiz.problems()[0]
            .sum_bits()
            .iter()
            .copied()
            .map(Some)
            .collect(),
    );

    let (carries, sums) = stored(&quiz, &block, 0);
    assert!(quiz.is_correct(&carries, &sums, 0));

    let (carries, sums) = stored(&quiz, &block, 1);
    assert!(!quiz.is_correct(&carries, &sums, 1));
}

fn stored(
    quiz: &BinaryAdditionQuiz,
    block: &BlockHandle<LogicGame>,
    problem: usize,
) -> (Vec<Option<bool>>, Vec<Option<bool>>) {
    let answers = block.read().and_then(|game| game.quiz(problem).cloned());
    let (carries, sums) = answers
        .map(|answers| (answers.carries, answers.sums))
        .unwrap_or_default();
    quiz.fit(problem, carries, sums)
}
