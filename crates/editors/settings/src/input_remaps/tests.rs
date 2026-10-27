use block_editor_beui::be_block::{InputRemapper, InputRemapperContent};
use block_editor_beui::beui::{Vec2, pos2};
use block_editor_beui::{EditorHost, OwnedEditor, RemapState, RemapsWatch};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use uuid::Uuid;

use crate::input_remaps::InputRemapsApp;

mod remaps_that_do_not_read_say_why_and_store_nothing;
mod remaps_written_as_text_name_this_plugins_remapper_once_applied;
mod the_editor_changes_rules_and_keeps_the_comments;

const SIZE: Vec2 = Vec2::new(800.0, 1400.0);

fn editor(state: RemapState, config: Option<&str>) -> BeuiTest<InputRemapsApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_watched::<RemapsWatch>(&state);
    let mut editor = BeuiTest::new(OwnedEditor::new(host, Uuid::new_v4())).with_size(SIZE);
    let mut content = InputRemapperContent::default();
    if let Some(config) = config {
        content.apply(&InputRemapper::set(
            "be3.settings",
            "remapper.wasm",
            Some(config.as_bytes().to_vec()),
        ));
    }
    editor.hold(None, content);
    editor.run();
    editor
}

fn content(editor: &BeuiTest<InputRemapsApp>) -> InputRemapperContent {
    editor.content(None)
}

fn stored(editor: &BeuiTest<InputRemapsApp>) -> String {
    content(editor)
        .root()
        .config
        .map(|config| String::from_utf8_lossy(&config).into_owned())
        .unwrap_or_default()
}

fn show_toml(editor: &mut BeuiTest<InputRemapsApp>) {
    let tabs = editor.rect_of("input-remaps.tabs");
    editor.click(pos2(tabs.min.x + tabs.width() * 0.75, tabs.center().y));
}

fn apply(editor: &mut BeuiTest<InputRemapsApp>, text: &str) {
    show_toml(editor);
    editor.click("input-remaps.text");
    editor.type_text(text);
    editor.click("input-remaps.apply");
}
