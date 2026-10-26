use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_typed_shortcut_leaves_the_text_input_that_has_the_focus_alone() {
    let _root = crate::reactive::RootScope::new();
    let runs = create_signal(0);
    let document = build_node(move || {
        Action::new("test.rectangle", "Rectangle", move || {
            runs.update(|runs| *runs += 1)
        })
        .shortcut(Chord::key(Key::R))
        .register();
        let value = create_signal(String::new());
        view! {
            <List spacing=0.0>
                <TextInput @test_id={"field"} value={value} label="Name" />
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
