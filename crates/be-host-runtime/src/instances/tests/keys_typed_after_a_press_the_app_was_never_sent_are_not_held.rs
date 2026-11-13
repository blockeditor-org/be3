use super::*;

#[test]
fn keys_typed_after_a_press_the_app_was_never_sent_are_not_held() {
    let mut instances = placed();
    let pass = host::pass();
    let app = progress((7, 7), true, false);
    assert_eq!(
        own_keys(&mut instances, app, true, vec![press()], pass).len(),
        1
    );
    assert_eq!(
        own_keys(&mut instances, app, true, vec![letter("a")], pass),
        [letter("a")],
        "no frame answers a press that never reached the app, so nothing waits for one"
    );
}
