use super::*;

#[test]
fn keys_typed_after_a_press_the_app_was_never_sent_are_not_held() {
    assert_eq!(deck(false, app(7, 7), vec![press()]).len(), 1);
    assert_eq!(
        deck(false, app(7, 7), vec![letter("a")]),
        [letter("a")],
        "no frame answers a press that never reached the app, so nothing waits for one"
    );
}
