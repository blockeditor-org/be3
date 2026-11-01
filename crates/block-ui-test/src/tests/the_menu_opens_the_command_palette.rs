use beui::Vec2;
use beui::styled::Root;
use block_editor_beui::beui::Modifiers;

use super::*;

struct PaletteApp;

#[component]
impl BeuiApp for PaletteApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Tidy editor={editor} />
            </Root>
        }
    }
}

#[component]
fn Tidy(editor: Editor) -> NodeId {
    let _ = editor;
    let tidied = create_signal(String::from("messy"));
    Action::new("tidy.up", "Tidy up", move || tidied.set("tidy".to_owned())).register();
    view! {
        <Text @test_id={"tidy.state"}>{tidied}</Text>
    }
}

#[test]
fn the_menu_opens_the_command_palette() {
    let mut test = BeuiTest::<PaletteApp>::editing(Uuid::new_v4())
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();
    assert!(!test.shown("command-palette.search"));

    test.pick_menu("editor.palette");
    assert!(test.shown("command-palette.search"));

    test.type_text("tidy");
    test.run();
    test.key(beui::Key::Enter, Modifiers::NONE);
    test.run();
    let node = test
        .document()
        .find_test_id("tidy.state")
        .expect("the text is shown");
    assert_eq!(
        test.document()
            .node_detail(node)
            .unwrap_or_default()
            .trim_matches('"'),
        "tidy"
    );
    assert!(!test.shown("command-palette.search"));
}
