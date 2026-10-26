use super::*;
use crate::KeyEvent;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn held_modifiers_and_key_releases_reach_global_listeners() {
    let _root = crate::reactive::RootScope::new();
    let seen = create_signal(Vec::new());
    let holds = create_signal(Vec::new());
    let document = build_node(move || {
        on_global_key(move |event: KeyEvent| {
            if let Some(press) = event.press() {
                seen.update(|seen| seen.push((press.key, press.pressed)));
            }
            false
        });
        let modifiers = held_modifiers();
        create_effect(move || holds.update(|holds| holds.push(modifiers.get())));
        view! {
            <List spacing=0.0>
                <TextInput @test_id={"field"} value="" />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("field")));

    harness.key_down(Key::Alt);
    harness.key(Key::Tab, Modifiers::ALT);
    harness.key_up(Key::Alt);

    assert_eq!(
        seen.get_untracked(),
        vec![
            (Key::Alt, true),
            (Key::Tab, true),
            (Key::Tab, false),
            (Key::Alt, false)
        ],
        "a listener sees the press and the release, though a text field has the focus"
    );
    assert_eq!(
        holds.get_untracked(),
        vec![Modifiers::NONE, Modifiers::ALT, Modifiers::NONE],
        "the held modifiers follow the keyboard"
    );
}
