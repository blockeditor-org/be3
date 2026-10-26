use beui::Vec2;
use beui::icons::ICON_WAND_SPARKLES;
use block_editor_beui::BarAction;
use block_editor_beui::beui::styled::Root;
use block_plugin_api::MenuCheck;

use super::*;

struct ActionApp;

#[component]
impl BeuiApp for ActionApp {
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
    let neat = create_signal(false);
    let tidy = Action::new("tidy.up", "Tidy up", move || tidied.set("tidy".to_owned()))
        .glyph(ICON_WAND_SPARKLES)
        .register();
    let neatly = Action::new("tidy.neat", "Keep it neat", move || {
        neat.update(|on| *on = !*on)
    })
    .checked(neat)
    .register();
    add_to_menu(vec![
        MenuNode::action(&tidy),
        MenuNode::submenu("tidy.more", "More", None, vec![MenuNode::action(&neatly)]),
    ]);
    view! {
        <Text @test_id={"tidy.state"}>{tidied}</Text>
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
    test.menu_ids()
}

const MENU: [&str; 9] = [
    "editor.undo",
    "editor.redo",
    "editor.rename",
    "editor.share",
    "editor.palette",
    "tidy.up",
    "tidy.more",
    "tidy.neat",
    "editor.details",
];

#[test]
fn the_frame_offers_one_menu_on_a_phone_and_a_desktop_and_runs_what_is_picked() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ActionApp>::new(OwnedEditor::new(host, block))
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

    assert_eq!(
        test.menu_entry("tidy.neat").check,
        MenuCheck::Checkbox(false)
    );
    test.pick_menu("tidy.neat");
    assert_eq!(
        test.menu_entry("tidy.neat").check,
        MenuCheck::Checkbox(true),
        "a command in a submenu runs, and its tick reaches the host"
    );

    test.pick_menu("editor.rename");
    assert_eq!(
        test.take_shell_commands(),
        [(
            block,
            block_plugin_api::ShellCommand::Rename { anchor: None }
        )]
    );

    test.pick_menu("editor.details");
    assert_eq!(test.take_bar_actions(), [BarAction::Details]);

    let desktop = BeuiTest::<ActionApp>::new(OwnedEditor::new(
        {
            let host = EditorHost::default();
            host.set_editable(true);
            host
        },
        block,
    ))
    .with_top_bar(false);
    assert_eq!(
        desktop.menu_ids(),
        MENU,
        "a desktop frame offers the same menu as a phone"
    );
    assert!(
        !desktop.shown("editor.name"),
        "a desktop frame draws no bar of its own"
    );
}
