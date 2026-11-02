use super::*;
use crate::logic_game::{LogicGameOperation, QuizRow};
use logicgame::challenges::CHALLENGES;

#[test]
fn solutions_and_quiz_answers_made_on_both_sides_merge_to_both() {
    let challenge = CHALLENGES[0];
    let (first, ours_solution, theirs_solution) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let empty = LogicGameContent::default();
    let base = edited(
        &empty,
        [empty.root().edit_for(&LogicGameOperation::InsertSolution {
            challenge,
            solution: first,
            index: 0,
        })],
    );
    let ours = edited(
        &base,
        [
            base.root().edit_for(&LogicGameOperation::InsertSolution {
                challenge,
                solution: ours_solution,
                index: 1,
            }),
            base.root().edit_for(&LogicGameOperation::SetQuizRow {
                problem: 0,
                row: QuizRow::Carries,
                values: vec![Some(true)],
            }),
        ],
    );
    let theirs = edited(
        &base,
        [
            base.root().edit_for(&LogicGameOperation::InsertSolution {
                challenge,
                solution: theirs_solution,
                index: 0,
            }),
            base.root().edit_for(&LogicGameOperation::SetQuizRow {
                problem: 0,
                row: QuizRow::Sums,
                values: vec![Some(false)],
            }),
            base.root().edit_for(&LogicGameOperation::SetCompleted {
                challenge,
                completed: true,
            }),
        ],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    let game = merged.root().game();
    let level = game.level(challenge).expect("every challenge has a level");
    assert_eq!(level.solutions, [theirs_solution, first, ours_solution]);
    assert!(level.completed);
    let quiz = game.quiz(0).expect("both rows were answered");
    assert_eq!(quiz.carries, [Some(true)]);
    assert_eq!(quiz.sums, [Some(false)]);
}
