use super::*;
use beui::styled::Root;

use beui::{Key, Modifiers};

struct ShortcutRoot;

#[component]
impl BeuiRoot for ShortcutRoot {
    #[component]
    fn View(_editor: Editor) -> Root {
        let pressed = create_signal(String::new());
        Action::new("test.slash", "Slash", move || pressed.set("Slash".to_owned()))
            .shortcut(Chord::ctrl(Key::Slash))
            .register();
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
    let mut test = BeuiTest::<ShortcutRoot>::editing(Uuid::new_v4());

    test.key(Key::Slash, Modifiers::CTRL);
    test.run();

    assert_eq!(test.label("shortcut.pressed"), "Slash");
}
