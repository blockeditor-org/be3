use beui::Vec2;
use beui::icons::ICON_AUTO_FIX_HIGH;
use beui::reactive::{Action, create_signal};
use block_editor_beui::{BarAction, BlockCommand};

use super::*;

struct ActionApp;

impl BeuiApp for ActionApp {
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
    .glyph(ICON_AUTO_FIX_HIGH)
    .in_menu()
    .register();
    view! {
        <Text string={tidied} @test_id={"tidy.state"} />
    }
}

fn text(test: &BeuiTest<ActionApp>, id: &str) -> String {
    let node = test.document().find_test_id(id).expect("the text is shown");
    test.document()
        .node_detail(node)
        .unwrap_or_default()
        .trim_matches('"')
        .to_owned()
}

fn ids(test: &BeuiTest<ActionApp>) -> Vec<String> {
    test.menu().iter().map(|entry| entry.id.clone()).collect()
}

const MENU: [&str; 7] = [
    "editor.undo",
    "editor.redo",
    "editor.rename",
    "editor.share",
    "editor.palette",
    "tidy.up",
    "editor.details",
];

#[test]
fn the_frame_offers_one_menu_on_a_phone_and_a_desktop_and_runs_what_is_picked() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ActionApp>::new(Editor::new(host, block))
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();
    assert_eq!(ids(&test), MENU, "the frame hands its parent its menu");
    assert!(test.menu_entry("tidy.up").enabled);
    assert!(
        !test.menu_entry("editor.undo").enabled,
        "nothing has been done to undo"
    );

    test.pick_menu("tidy.up");
    assert_eq!(
        text(&test, "tidy.state"),
        "tidy",
        "the editor's own item runs"
    );

    test.pick_menu("editor.rename");
    assert_eq!(test.take_block_commands(), [(block, BlockCommand::Rename)]);

    test.pick_menu("editor.details");
    assert_eq!(test.take_bar_actions(), [BarAction::Details]);

    let desktop = BeuiTest::<ActionApp>::new(Editor::new(
        {
            let host = EditorHost::default();
            host.set_editable(true);
            host
        },
        block,
    ))
    .with_top_bar(false);
    assert_eq!(
        desktop
            .menu()
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<Vec<_>>(),
        MENU,
        "a desktop frame offers the same menu as a phone"
    );
    assert!(
        !desktop.shown("editor.name"),
        "a desktop frame draws no bar of its own"
    );
}
