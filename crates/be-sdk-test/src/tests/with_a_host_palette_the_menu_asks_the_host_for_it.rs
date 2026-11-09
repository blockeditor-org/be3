use be_sdk_api::PaletteMode;
use beui::Vec2;
use beui::styled::Root;
use beui::{Key, Modifiers};

use super::*;

struct PaletteRoot;

#[component]
impl BeuiRoot for PaletteRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        view! {
            <Root>
                <Text>"Nothing here"</Text>
            </Root>
        }
    }
}

#[test]
fn with_a_host_palette_the_menu_asks_the_host_for_it() {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_host_palette(true);
    let mut test = BeuiTest::<PaletteRoot>::new(OwnedEditor::new(host, Uuid::new_v4()))
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();

    test.pick_menu("editor.palette");
    assert_eq!(test.take_palette_opens(), vec![PaletteMode::Actions]);
    assert!(
        !test.shown("command-palette.search"),
        "the frame draws no palette of its own"
    );

    test.key(
        Key::P,
        Modifiers {
            shift: true,
            ..Modifiers::CTRL
        },
    );
    test.run();
    assert!(
        !test.shown("command-palette.search") && test.take_palette_opens().is_empty(),
        "nor takes Ctrl+Shift+P, which the host hears first"
    );
    assert!(
        !test
            .described_actions()
            .iter()
            .any(|action| action.id == "editor.palette"),
        "and lists no palette among the actions the host's palette shows"
    );
}
