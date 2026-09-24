use crate::logic_game::{LogicGameContent, LogicGameOperation, QuizRow};
use crate::{BlockRef, ChildChange, Root};
use logicgame::challenges::CHALLENGES;
use uuid::Uuid;

fn run(content: &mut LogicGameContent, operation: LogicGameOperation) {
    let edit = content.root().edit_for(&operation);
    content.apply(&edit);
}

#[test]
fn logic_game_solutions_keep_their_order_per_challenge() {
    let [first, second] = [CHALLENGES[0], CHALLENGES[1]];
    let [a, b, c, other] = [(); 4].map(|_| BlockRef::Direct(Uuid::new_v4()));
    let mut game = LogicGameContent::default();
    for (challenge, solution, index) in [
        (first, a, 0),
        (second, other, 0),
        (first, c, 1),
        (first, b, 1),
        (first, a, 0),
    ] {
        run(
            &mut game,
            LogicGameOperation::InsertSolution {
                challenge,
                solution,
                index,
            },
        );
    }
    let view = game.root().game();
    assert_eq!(view.levels().len(), CHALLENGES.len());
    assert_eq!(view.level(first).unwrap().solutions, [a, b, c]);
    assert_eq!(view.level(second).unwrap().solutions, [other]);

    run(
        &mut game,
        LogicGameOperation::SetCompleted {
            challenge: first,
            completed: true,
        },
    );
    run(
        &mut game,
        LogicGameOperation::SetQuizRow {
            problem: 2,
            row: QuizRow::Sums,
            values: vec![Some(true), None],
        },
    );
    run(
        &mut game,
        LogicGameOperation::RemoveSolution {
            challenge: first,
            solution: b,
        },
    );
    let view = game.root().game();
    assert!(view.level(first).unwrap().completed);
    assert_eq!(view.level(first).unwrap().solutions, [a, c]);
    assert_eq!(view.quiz(2).unwrap().sums, [Some(true), None]);

    let a_id = a.as_direct().unwrap();
    let replaced = Uuid::new_v4();
    let edit = game
        .root()
        .child_edit(ChildChange::Replace {
            old: a_id,
            new: replaced,
        })
        .unwrap();
    game.apply(&edit);
    assert_eq!(
        game.root().game().level(first).unwrap().solutions,
        [BlockRef::Direct(replaced), c]
    );
}
