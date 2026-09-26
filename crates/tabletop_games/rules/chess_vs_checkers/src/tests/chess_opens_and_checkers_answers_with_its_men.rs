use super::*;

#[test]
fn chess_opens_and_checkers_answers_with_its_men() {
    assert_eq!(labels(&[], FIRST).len(), 20);

    let actions = played(&[(FIRST, "e4")]);
    let answers = labels(&actions, SECOND);

    assert_eq!(answers.len(), 7);
    assert!(answers.contains(&"bc5".to_owned()));
    assert!(answers.contains(&"dc5".to_owned()));
    assert_eq!(show(&actions, SECOND).description, "Your move (Checkers)");
}
