use uuid::Uuid;

use super::{game, play};

#[test]
fn a_session_takes_one_move_at_a_time_and_shows_anyone() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let mut session = game().start().expect("the game starts");

    let first = play(&[], x, 4);
    session.play(&first).expect("x moves");
    assert_eq!(
        session.show(x).expect("x looks").description,
        "Waiting for O..."
    );
    assert_eq!(
        session.show(o).expect("o looks").description,
        "Your turn (O)"
    );

    let second = play(std::slice::from_ref(&first), o, 0);
    session.play(&second).expect("o moves");

    let screen = session.show(x).expect("x looks again");
    assert_eq!(screen.description, "Your turn (X)");
    assert_eq!(screen.actions.len(), 7);
    assert_eq!(screen.history.len(), 2);
    assert_eq!(session.played(), [first, second]);
}
