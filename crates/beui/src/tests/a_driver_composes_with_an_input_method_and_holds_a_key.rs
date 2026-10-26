use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_driver_composes_with_an_input_method_and_holds_a_key() {
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
        .ask(&["app", "ime", "compose", "ni"])
        .expect("the composition starts");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(state.contains("composing \"ni\""), "{state}");
    let changed = driven
        .ask(&["app", "ime", "commit", "你"])
        .expect("the composition is committed");
    assert!(changed.contains("value=\"你\""), "{changed}");

    driven.ask(&["app", "keydown", "a"]).expect("a is pressed");
    driven.ask(&["app", "keydown", "a"]).expect("a repeats");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(state.contains("keys down A"), "{state}");
    let changed = driven.ask(&["app", "keyup", "a"]).expect("a is let go");
    assert!(
        driven.ask(&["app", "keyup", "a"]).is_err(),
        "a is no longer held"
    );
    let tree = driven.ask(&["app", "tree"]).expect("the tree is read");
    assert!(
        tree.contains("value=\"你aa\""),
        "a press and its repeat typed twice:\n{tree}\n{changed}"
    );
}
