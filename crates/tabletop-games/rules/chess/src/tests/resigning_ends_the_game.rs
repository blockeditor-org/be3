use super::*;

#[test]
fn resigning_ends_the_game() {
    let opened = played(&[(FIRST, "e4"), (SECOND, "e5")]);
    assert_eq!(labels(&opened, SECOND), ["Resign"]);

    let actions = played(&[(FIRST, "e4"), (SECOND, "e5"), (FIRST, "Resign")]);

    assert_eq!(show(&actions, FIRST).description, "You resigned");
    assert_eq!(
        show(&actions, Uuid::from_u128(3)).description,
        "White resigned - Black wins"
    );
    let history = show(&actions, SECOND).history;
    assert_eq!(
        history.last().map(|turn| turn.description.as_str()),
        Some("resigns")
    );
}
