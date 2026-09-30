use beui::Vec2;
use beui::icons::ICON_AUTO_FIX_HIGH;
use beui::reactive::create_signal;
use block_editor_beui::{BarAction, BlockCommand, bar_item};

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
    bar_item(
        "Tidy up",
        ICON_AUTO_FIX_HIGH,
        create_memo(|| false),
        move || set_tidied.set("tidy".to_owned()),
    );
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

#[test]
fn the_parent_opens_the_phone_more_sheet_and_hears_when_it_closes() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ActionApp>::new(Editor::new(host, block))
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();
    assert!(
        !test.shown("editor.name"),
        "a phone draws no bar of its own"
    );
    assert!(!test.shown("editor.more.rename"));

    test.set_more(true);
    assert!(
        test.shown("editor.more.rename"),
        "the parent opens the sheet"
    );
    test.click("editor.more.item.0");
    test.run();
    assert_eq!(
        text(&test, "tidy.state"),
        "tidy",
        "the editor's own item runs"
    );
    assert!(
        !test.shown("editor.more.rename"),
        "running an item closes the sheet"
    );
    assert_eq!(test.take_bar_actions(), [BarAction::CloseMore]);

    test.set_more(false);
    test.set_more(true);
    test.click("editor.more.rename");
    test.run();
    assert_eq!(test.take_block_commands(), [(block, BlockCommand::Rename)]);
    assert_eq!(test.take_bar_actions(), [BarAction::CloseMore]);

    test.set_more(false);
    test.set_more(true);
    test.click("editor.more.details");
    test.run();
    assert_eq!(
        test.take_bar_actions(),
        [BarAction::CloseMore, BarAction::Details]
    );
}
