use be_sdk_beui::be_block::{InputRemapper, InputRemapperContent};
use be_sdk_beui::beui::{Key, Modifiers, Vec2, pos2};
use be_sdk_beui::{EditorHost, OwnedEditor, RemapState, RemapsWatch};
use be_sdk_test::BeuiTest;
use uuid::Uuid;

use crate::input_remaps::InputRemapsRoot;

mod a_chord_pressed_while_the_picker_listens_is_what_is_pressed;
mod a_held_modifier_and_a_tap_held_for_a_preset_write_the_rule;
mod a_new_remap_is_nothing_to_nothing_until_keys_are_picked;
mod a_program_group_comes_from_a_window_open_now;
mod a_sequence_is_built_from_taps_and_waits;
mod a_typed_key_is_used_only_once_it_reads;
mod listening_ends_when_its_picker_closes;
mod picking_what_is_pressed_from_all_keys_writes_its_name;
mod remaps_show_as_keys_in_a_list;
mod remaps_that_do_not_read_say_why_and_store_nothing;
mod remaps_written_as_text_name_this_apps_remapper_once_applied;
mod the_editor_changes_rules_and_keeps_the_comments;
mod the_picker_opens_from_the_top_of_a_long_page;

const CAPS_TO_ESCAPE: &str =
    "[[group]]\nname = \"Everywhere\"\nrules = [{ on = \"CapsLock\", send = \"Escape\" }]\n";

const SIZE: Vec2 = Vec2::new(800.0, 1400.0);

fn editor(state: RemapState, config: Option<&str>) -> BeuiTest<InputRemapsRoot> {
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

fn content(editor: &BeuiTest<InputRemapsRoot>) -> InputRemapperContent {
    editor.content(None)
}

fn stored(editor: &BeuiTest<InputRemapsRoot>) -> String {
    content(editor)
        .root()
        .config
        .map(|config| String::from_utf8_lossy(&config).into_owned())
        .unwrap_or_default()
}

fn show_toml(editor: &mut BeuiTest<InputRemapsRoot>) {
    editor.click("input-remaps.toml");
    editor.run();
}

/// Chooses what an open rule does: 0 acts as a key, 1 taps, 2 plays a sequence,
/// with the arrow keys from the first choice, as radio buttons take them.
fn choose_does(editor: &mut BeuiTest<InputRemapsRoot>, rule: &str, does: usize) {
    let rect = editor.rect_of(&format!("{rule}.does"));
    editor.click(pos2(rect.min.x + 12.0, rect.center().y));
    editor.run();
    for _ in 0..does {
        editor.key(Key::ArrowRight, Modifiers::NONE);
        editor.run();
    }
}

/// Shows the open picker's All keys, the last of its tabs after Press it and Mouse.
fn all_keys(editor: &mut BeuiTest<InputRemapsRoot>) {
    let rect = editor.rect_of("input-remaps.picker.tabs");
    editor.click(pos2(rect.min.x + 12.0, rect.center().y));
    editor.run();
    for _ in 0..2 {
        editor.key(Key::ArrowRight, Modifiers::NONE);
        editor.run();
    }
}

/// Opens a rule, and picks its key from All keys.
fn pick_from_all_keys(editor: &mut BeuiTest<InputRemapsRoot>, opener: &str, name: &str) {
    editor.click(opener);
    editor.run();
    all_keys(editor);
    editor.click(format!("input-remaps.picker.key.{name}").as_str());
    editor.run();
}

fn apply(editor: &mut BeuiTest<InputRemapsRoot>, text: &str) {
    show_toml(editor);
    editor.click("input-remaps.text");
    editor.run();
    editor.type_text(text);
    editor.run();
    editor.click("input-remaps.apply");
    editor.run();
}
