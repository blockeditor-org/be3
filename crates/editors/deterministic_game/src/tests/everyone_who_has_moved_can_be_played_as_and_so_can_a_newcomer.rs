use super::*;

#[test]
fn everyone_who_has_moved_can_be_played_as_and_so_can_a_newcomer() {
    let moved = |actor| GameAction {
        actor,
        action: Vec::new(),
    };

    let alone = seats(ACCOUNT, &[]);
    assert_eq!(alone.len(), 2);
    assert_eq!(alone[0], ACCOUNT);

    let newcomer = alone[1];
    let together = seats(
        ACCOUNT,
        &[
            moved(OPPONENT),
            moved(ACCOUNT),
            moved(newcomer),
            moved(OPPONENT),
        ],
    );
    assert_eq!(together[..3], [ACCOUNT, OPPONENT, newcomer]);
    assert_eq!(together.len(), 4);
    assert!(!together[..3].contains(&together[3]));
}
