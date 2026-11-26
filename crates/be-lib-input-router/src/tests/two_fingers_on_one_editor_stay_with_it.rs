use super::*;

#[test]
fn two_fingers_on_one_editor_stay_with_it() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, touch(1, TouchPhase::Start, 250.0, 250.0));
    assert_eq!(
        targets(&router.route(&test, touch(2, TouchPhase::Start, 300.0, 300.0))),
        vec![EDITOR]
    );
}
