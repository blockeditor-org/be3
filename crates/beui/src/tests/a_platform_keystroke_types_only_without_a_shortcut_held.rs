use super::*;
use beui_core::input::KeyPress;
use beui_core::runner::Keystroke;

#[test]
fn a_platform_keystroke_types_only_without_a_shortcut_held() {
    let mut intaken = Intaken::new();
    let typed = |key, text: &str| Keystroke {
        key: Some(key),
        pressed: true,
        repeat: false,
        code: None,
        text: Some(text.to_owned()),
    };

    let seen = intaken.frame(1.0, [typed(Key::A, "a").into()]);
    assert_eq!(
        seen,
        [Event::Key {
            press: KeyPress::down(Key::A, Modifiers::NONE),
            code: None,
            text: Some("a".to_owned()),
        }]
    );

    let seen = intaken.frame(
        1.0,
        [
            Event::Modifiers(Modifiers::CTRL).into(),
            Event::Modifiers(Modifiers::CTRL).into(),
            Keystroke {
                repeat: true,
                ..typed(Key::Ctrl, "")
            }
            .into(),
            typed(Key::S, "s").into(),
        ],
    );
    assert_eq!(
        seen,
        [
            Event::Modifiers(Modifiers::CTRL),
            Event::Key {
                press: KeyPress::down(Key::S, Modifiers::CTRL),
                code: None,
                text: None,
            }
        ],
        "a held modifier is told once, does not repeat, and turns typing into a shortcut"
    );

    let seen = intaken.frame(
        1.0,
        [
            Event::Modifiers(Modifiers::NONE).into(),
            Keystroke {
                key: None,
                ..typed(Key::A, "é")
            }
            .into(),
        ],
    );
    assert_eq!(
        seen,
        [
            Event::Modifiers(Modifiers::NONE),
            Event::Text("é".to_owned())
        ],
        "a key beui does not know still types its text"
    );
}
