use super::*;

#[test]
fn a_capture_must_be_taken() {
    let actions = played(&[(FIRST, "11-15"), (SECOND, "22-18")]);

    assert_eq!(labels(&actions, FIRST), ["15x22", "Resign"]);

    let actions = played(&[(FIRST, "11-15"), (SECOND, "22-18"), (FIRST, "15x22")]);
    let history: Vec<String> = show(&actions, SECOND)
        .history
        .into_iter()
        .map(|turn| turn.description)
        .collect();
    assert_eq!(history, ["11-15", "22-18", "15x22"]);
}
