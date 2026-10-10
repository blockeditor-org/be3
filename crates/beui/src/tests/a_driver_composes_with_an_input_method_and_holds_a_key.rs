use super::*;
use crate::reactive::{build, create_signal, view};
use crate::styled::TextInput;

#[test]
fn a_driver_composes_with_an_input_method_and_holds_a_key() {
    let document = build(|| {
        let (text, set_text) = create_signal(String::new());
        view! {
            <TextInput
                @test_id={"field"}
                value={text}
                on_change={move |next: String| set_text.set(next)}
            />
        }
    });
    let mut driven = Driven::new(document);
    driven
        .ask(&["click", "#field"])
        .expect("the field is focused");

    driven
        .ask(&["ime", "compose", "ni"])
        .expect("the composition starts");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(state.contains("composing \"ni\""), "{state}");
    let changed = driven
        .ask(&["ime", "commit", "你"])
        .expect("the composition is committed");
    assert!(changed.contains("value=\"你\""), "{changed}");

    driven.ask(&["keydown", "a"]).expect("a is pressed");
    driven.ask(&["keydown", "a"]).expect("a repeats");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(state.contains("keys down A"), "{state}");
    let changed = driven.ask(&["keyup", "a"]).expect("a is let go");
    assert!(driven.ask(&["keyup", "a"]).is_err(), "a is no longer held");
    let tree = driven.ask(&["tree"]).expect("the tree is read");
    assert!(
        tree.contains("value=\"你aa\""),
        "a press and its repeat typed twice:\n{tree}\n{changed}"
    );
}
