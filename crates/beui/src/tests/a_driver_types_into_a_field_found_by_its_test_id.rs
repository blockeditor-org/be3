use super::*;
use crate::reactive::{build, create_signal, view};
use crate::styled::TextInput;

#[test]
fn a_driver_types_into_a_field_found_by_its_test_id() {
    let document = build(|| {
        let (name, set_name) = create_signal(String::new());
        view! {
            <TextInput
                @test_id={"form.name"}
                value={name}
                placeholder="Name"
                on_change={move |next: String| set_name.set(next)}
            />
        }
    });
    let mut driven = Driven::new(document);

    assert!(
        driven
            .ask(&["ids"])
            .expect("the driver lists test ids")
            .starts_with("#form.name at "),
        "the field's test id is on screen"
    );
    driven
        .ask(&["click", "#form.name"])
        .expect("the field is clicked");
    let changed = driven.ask(&["type", "Ada"]).expect("the text is typed");
    assert!(
        changed.contains("value=\"Ada\""),
        "the field reads what was typed:\n{changed}"
    );
    driven
        .ask(&["key", "ctrl+a", "Backspace"])
        .expect("the chords are pressed");
    driven
        .ask(&["gone", "value=\"Ada\""])
        .expect("the text is deleted");
}
