use super::*;

#[test]
fn only_you_and_the_guests_you_brought_can_be_played_as() {
    let moved = |actor| GameAction {
        actor,
        action: Vec::new(),
    };

    let alone = seats(ACCOUNT, &[]);
    assert_eq!(alone.len(), 2);
    assert_eq!(alone[0], ACCOUNT);

    let guest = alone[1];
    let together = seats(
        ACCOUNT,
        &[
            moved(OPPONENT),
            moved(ACCOUNT),
            moved(guest),
            moved(OPPONENT),
        ],
    );
    assert_eq!(together[..2], [ACCOUNT, guest]);
    assert_eq!(together.len(), 3);
    assert!(!together.contains(&OPPONENT));
    assert!(!together[..2].contains(&together[2]));
}
