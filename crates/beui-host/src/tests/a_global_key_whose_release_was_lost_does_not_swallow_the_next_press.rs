use super::*;
use crate::KeyAt;
use crate::KeyPress;
use crate::reactive::*;

#[test]
fn a_global_key_whose_release_was_lost_does_not_swallow_the_next_press() {
    let _root = crate::reactive::RootScope::new();
    let seen = create_signal(Vec::new());
    let document = build_node(move || {
        Action::new("test.global", "Global", || {})
            .shortcut(Chord::logo(Key::G))
            .at(KeyAt::Window)
            .capture()
            .register();
        on_shortcut(move |press: KeyPress| {
            seen.update(|seen| seen.push((press.key, press.pressed)));
            false
        });
        view! {
            <List spacing=0.0>
                <unstyled::Button>
                    <ButtonFace label="Button" />
                </unstyled::Button>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![
        Input::Modifiers(Modifiers::LOGO),
        key_event(Key::G, true),
    ]);
    harness.frame(vec![Input::Focus(false)]);
    harness.key(Key::G, Modifiers::NONE);

    assert_eq!(
        seen.get_untracked(),
        [(Key::G, true), (Key::G, false)],
        "the G after focus was lost reaches the document whole"
    );
}
