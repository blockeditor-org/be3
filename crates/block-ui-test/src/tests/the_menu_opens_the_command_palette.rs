use beui::Vec2;
use beui::reactive::{Action, create_signal};

use super::*;

struct PaletteApp;

impl BeuiApp for PaletteApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Tidy editor={editor} />
        }
    }
}

#[component]
fn Tidy(editor: Editor) -> NodeId {
    let _ = editor;
    let (tidied, set_tidied) = create_signal(String::from("messy"));
    Action::new("tidy.up", "Tidy up", move || {
        set_tidied.set("tidy".to_owned())
    })
    .register();
    view! {
        <Text string={tidied} @test_id={"tidy.state"} />
    }
}

#[test]
fn the_menu_opens_the_command_palette() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<PaletteApp>::new(Editor::new(host, Uuid::new_v4()))
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();
    assert!(!test.shown("command-palette.search"));

    test.pick_menu("editor.palette");
    assert!(test.shown("command-palette.search"));

    test.text("tidy");
    test.run();
    test.key_press(beui::Key::Enter);
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
