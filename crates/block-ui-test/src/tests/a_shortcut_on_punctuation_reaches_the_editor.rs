use super::*;
use beui::styled::Root;

use beui::{Key, KeyPress, Modifiers};

struct ShortcutApp;

#[component]
impl BeuiApp for ShortcutApp {
    #[component]
    fn View(_editor: Editor) -> Root {
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
            <Root>
                <Text @test_id={"shortcut.pressed"}>{text}</Text>
            </Root>
        }
    }
}

#[test]
fn a_shortcut_on_punctuation_reaches_the_editor() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ShortcutApp>::new(OwnedEditor::new(host, Uuid::new_v4()));

    test.key(Key::Slash, Modifiers::CTRL);

    assert_eq!(test.label("shortcut.pressed"), "Slash");
}
