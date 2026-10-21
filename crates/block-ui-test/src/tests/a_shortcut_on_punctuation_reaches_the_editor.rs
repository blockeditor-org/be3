use super::*;

use beui::{Key, KeyPress, Modifiers};

struct ShortcutApp;

impl BeuiApp for ShortcutApp {
    fn view(_editor: Editor) -> NodeId {
        let pressed = create_signal(String::new());
        on_shortcut(move |press: KeyPress| {
            if !press.pressed || !press.modifiers.ctrl {
                return false;
            }
            pressed.set(format!("{:?}", press.key));
            true
        });
        let text = create_memo(move || pressed.get());
        view! {
            <Text @test_id={"shortcut.pressed"}>{text}</Text>
        }
    }
}

#[test]
fn a_shortcut_on_punctuation_reaches_the_editor() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ShortcutApp>::new(OwnedEditor::new(host, Uuid::new_v4()));

    test.key_press_modifiers(Modifiers::CTRL, Key::Slash);
    test.run();

    assert_eq!(test.label("shortcut.pressed"), "Slash");
}
