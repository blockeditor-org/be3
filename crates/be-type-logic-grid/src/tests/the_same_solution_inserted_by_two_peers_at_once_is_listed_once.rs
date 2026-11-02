use super::*;
use crate::logic_game::LogicGameOperation;
use logicgame::challenges::CHALLENGES;

#[test]
fn the_same_solution_inserted_by_two_peers_at_once_is_listed_once() {
    let challenge = CHALLENGES[0];
    let solution = Uuid::new_v4();
    let base = LogicGameContent::default();
    let insert = LogicGameOperation::InsertSolution {
        challenge,
        solution,
        index: 0,
    };
    let ours = base.root().edit_for(&insert);
    let theirs = base.root().edit_for(&insert);

    let sequenced = edited(&base, [ours, theirs]);

    let game = sequenced.root().game();
    assert_eq!(
        game.level(challenge).map(|level| level.solutions.clone()),
        Some(vec![solution])
    );
}
