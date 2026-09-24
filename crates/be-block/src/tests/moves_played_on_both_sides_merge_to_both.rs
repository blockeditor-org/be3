use super::*;
use crate::{DeterministicGame, DeterministicGameContent};
use uuid::Uuid;

#[test]
fn moves_played_on_both_sides_merge_to_both() {
    let (module, alice, bob) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let base = DeterministicGameContent::new(&DeterministicGame::of(module));
    let ours = edited(&base, [DeterministicGame::play(alice, vec![1])]);
    let theirs = edited(&base, [DeterministicGame::play(bob, vec![2])]);

    let merged = match Merge::merge3(&base, &ours, &theirs) {
        be_commit::MergeResult::Clean(merged) => merged,
        other => panic!("expected a clean merge, got {other:?}"),
    };

    let mut moves: Vec<(Uuid, Vec<u8>)> = merged
        .root()
        .moves
        .iter()
        .map(|played| (played.actor, played.action.clone()))
        .collect();
    moves.sort();
    let mut expected = vec![(alice, vec![1]), (bob, vec![2])];
    expected.sort();
    assert_eq!(moves, expected);
    assert_eq!(BlockContent::references(&merged), [module]);
}
