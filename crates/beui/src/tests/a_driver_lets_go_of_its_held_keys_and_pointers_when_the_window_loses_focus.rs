use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_driver_lets_go_of_its_held_keys_and_pointers_when_the_window_loses_focus() {
    let document = build_node(|| {
        let text = create_signal(String::new());
        view! {
            <TextInput @test_id={"field"} value={text} />
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));
    driven
        .ask(&["app", "click", "#field"])
        .expect("the field is focused");
    driven
        .ask(&["app", "keydown", "ctrl", "a"])
        .expect("keys are held");
    driven
        .ask(&["app", "down", "#field"])
        .expect("the mouse button is held");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(state.contains("keys down ctrl, A"), "{state}");
    assert!(state.contains("holding Primary"), "{state}");

    driven.ask(&["screen", "blur"]).expect("the window loses focus");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(
        !state.contains("keys down"),
        "the runner lets go of held modifiers when the window loses focus, and so does the \
         driver:\n{state}"
    );
    assert!(
        !state.contains("holding"),
        "a window blur cancels the pointers, and the driver lets go of its button:\n{state}"
    );
    assert!(
        driven.ask(&["app", "keyup", "a"]).is_err(),
        "a key held when the window lost focus is no longer held"
    );

    driven.ask(&["screen", "focus"]).expect("the window gains focus");
    driven
        .ask(&["app", "click", "#field"])
        .expect("the field is focused");
    let changed = driven.ask(&["app", "type", "b"]).expect("b is typed");
    assert!(
        changed.contains("value=\"b\""),
        "a key typed after focus comes back carries no modifier from before:\n{changed}"
    );
}
