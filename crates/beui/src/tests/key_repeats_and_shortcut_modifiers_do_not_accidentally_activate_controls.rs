use super::*;
use crate::reactive::*;

#[test]
fn key_repeats_and_shortcut_modifiers_do_not_accidentally_activate_controls() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <LabelledButton label="Click" on_click={move || clicks.update(|clicks| *clicks += 1)} />
        }]
    });
    let mut harness = harness(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::CTRL);
    harness.key(Key::Space, Modifiers::ALT);
    harness.frame(vec![Event::key(beui::KeyPress {
        key: Key::Space,
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    })]);
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get_untracked(), 0);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    for _ in 0..3 {
        harness.frame(vec![Event::key(beui::KeyPress {
            key: Key::Space,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        })]);
    }
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get_untracked(), 1);
}
