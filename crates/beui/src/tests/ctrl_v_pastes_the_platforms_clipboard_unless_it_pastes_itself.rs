use super::*;
use beui_core::runner::Keystroke;

#[test]
fn ctrl_v_pastes_the_platforms_clipboard_unless_it_pastes_itself() {
    let mut intaken = Intaken::new();
    intaken.platform.clipboard = Some("pasted".to_owned());
    let paste = || {
        [
            Event::Modifiers(Modifiers::CTRL).into(),
            Keystroke {
                key: Some(Key::V),
                pressed: true,
                ..Keystroke::default()
            }
            .into(),
        ]
    };

    let seen = intaken.frame(1.0, paste());
    assert!(seen.contains(&Event::Text("pasted".to_owned())), "{seen:?}");

    intaken.platform.delivers_paste = true;
    let seen = intaken.frame(1.0, paste());
    assert!(
        !seen.iter().any(|event| matches!(event, Event::Text(_))),
        "a platform that pastes on its own is not pasted into twice: {seen:?}"
    );
}
