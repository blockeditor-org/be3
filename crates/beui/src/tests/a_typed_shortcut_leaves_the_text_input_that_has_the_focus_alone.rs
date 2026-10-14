use super::*;
use crate::reactive::{Action, Chord, build, create_signal, view};
use crate::styled::TextInput;

#[test]
fn a_typed_shortcut_leaves_the_text_input_that_has_the_focus_alone() {
    let _root = crate::reactive::RootScope::new();
    let (runs, set_runs) = create_signal(0);
    let document = build(move || {
        Action::new("test.rectangle", "Rectangle", move || {
            set_runs.update(|runs| *runs += 1)
        })
        .shortcut(Chord::key(Key::R))
        .register();
        let (value, set_value) = create_signal(String::new());
        view! {
            <List spacing=0.0>
                <TextInput
                    @test_id={"field"}
                    value
                    label="Name"
                    on_change={move |text: String| set_value.set(text)}
                />
                <unstyled::Button @test_id={"button"}>
                    <ButtonFace label="Elsewhere" />
                </unstyled::Button>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(harness.find("field")));
    harness.key(Key::R, Modifiers::NONE);
    assert_eq!(runs.get_untracked(), 0);

    harness.click(harness.center(harness.find("button")));
    harness.key(Key::R, Modifiers::NONE);
    assert_eq!(runs.get_untracked(), 1);
}
