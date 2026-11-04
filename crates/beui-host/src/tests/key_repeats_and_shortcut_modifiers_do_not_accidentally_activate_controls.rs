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
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::CTRL);
    harness.key(Key::Space, Modifiers::ALT);
    harness.frame(vec![Input::Key(beui::Keystroke {
        key: Some(Key::Space),
        pressed: true,
        repeat: true,
        ..Default::default()
    })]);
    harness.frame(vec![key_event(Key::Space, false)]);
    assert_eq!(clicks.get_untracked(), 0);
    harness.frame(vec![key_event(Key::Space, true)]);
    for _ in 0..3 {
        harness.frame(vec![Input::Key(beui::Keystroke {
            key: Some(Key::Space),
            pressed: true,
            repeat: true,
            ..Default::default()
        })]);
    }
    harness.frame(vec![key_event(Key::Space, false)]);
    assert_eq!(clicks.get_untracked(), 1);
}
