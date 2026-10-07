use beui::Key;
use block_editor_beui::PeerPresence;
use block_editor_beui::be_block::TextContent;
use block_editor_beui::be_block::block_url::block_url;
use block_editor_beui::be_block::presence::{PresenceColor, PresenceKind};
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::TextApp;
use crate::app::embeds::{image_embed_directive, parse_embeds};
use crate::presence::TextCursor;

mod a_peers_caret_and_selection_are_drawn_in_their_color;
mod a_phone_formats_from_a_bar_above_the_keyboard;
mod a_tap_near_the_bottom_places_the_caret_while_the_format_bar_opens;
mod classifies_markdown_image;
mod clicking_into_the_text_shows_the_format_bar_without_a_keyboard_button;
mod code_is_painted_on_its_background;
mod foreign_workspace_url_is_not_an_embed;
mod image_embed_directive_uses_markdown_image;
mod image_embed_directive_uses_plain_url;
mod markdown_is_painted_with_its_styles;
mod replacing_a_referenced_block_rewrites_its_url;
mod switching_to_hex_view_shows_the_bytes;
mod the_caret_is_shown_to_peers_in_a_color_they_are_not_using;
mod the_intrinsic_size_follows_the_width_it_was_given;
mod the_palette_and_the_shortcuts_run_the_format_actions;
mod typing_continues_at_the_caret_after_a_peer_edits_around_it;
mod typing_continues_where_a_peer_deleted_the_text_around_the_caret;
mod typing_inserts_text_into_the_document;

fn editor(text: &str) -> BeuiTest<TextApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, TextContent::from(text));
    editor.run();
    editor
}

fn text(editor: &BeuiTest<TextApp>) -> String {
    editor.content::<TextContent>(None).text()
}

const BLOCK_ID: Uuid = Uuid::from_u128(0xe2b8_7b59_9c69_4d75_83fd_801b_2727_1388);
const WORKSPACE_ID: Uuid = Uuid::from_u128(0x7a20_a314_e4aa_4ca7_b7ae_d68c_3249_0d9d);

fn caret_at(editor: &mut BeuiTest<TextApp>, index: usize) {
    editor.click("text.surface");
    editor.run();
    editor.key_press(Key::Home);
    editor.run();
    for _ in 0..index {
        editor.key_press(Key::ArrowRight);
        editor.run();
    }
}

fn peer(client: u64, anchor: usize, focus: usize, color: PresenceColor) -> PeerPresence {
    let position = |index: usize| serde_json::json!({ "left": null, "right": null, "fallback": index, "end": false });
    let cursor = serde_json::json!({
        "anchor": position(anchor),
        "focus": position(focus),
        "color": color,
    });
    PeerPresence {
        client,
        kind: TextCursor::ID,
        value: serde_json::to_vec(&cursor).unwrap(),
    }
}

fn shown_cursor(editor: &mut BeuiTest<TextApp>) -> Option<Option<serde_json::Value>> {
    editor
        .take_shown_presence()
        .into_iter()
        .rev()
        .find(|shown| shown.kind == TextCursor::ID)
        .map(|shown| {
            shown
                .value
                .map(|value| serde_json::from_slice(&value).unwrap())
        })
}
