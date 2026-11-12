use be_sdk_beui::PeerPresence;
use be_sdk_beui::be_block::block_url::block_url;
use be_sdk_beui::be_block::presence::{PresenceColor, PresenceKind};
use be_sdk_beui::beui::Modifiers;
use be_sdk_beui::{EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;
use beui::Key;
use uuid::Uuid;

use crate::presence::TextCursor;
use crate::root::TextRoot;
use crate::root::embeds::{image_embed_directive, parse_embeds};
use be_type_text::{TextBlock, TextContent};

mod a_link_to_a_block_you_cannot_see_reads_no_access_beside_one_you_can;
mod a_long_text_fades_at_the_edges_it_scrolls_past_beside_its_scrollbar;
mod a_paste_soon_after_typing_is_undone_on_its_own;
mod a_peers_caret_and_selection_are_drawn_in_their_color;
mod a_peers_edit_in_the_middle_of_a_word_leaves_it_one_undo_step;
mod a_phone_formats_from_a_bar_above_the_keyboard;
mod a_reopened_text_opens_scrolled_where_it_was_and_keeps_its_caret;
mod a_rewrite_refused_while_read_only_leaves_the_text_as_it_was;
mod a_table_rule_is_drawn_only_where_its_row_has_a_pipe;
mod a_tap_near_the_bottom_places_the_caret_while_the_format_bar_opens;
mod a_wide_document_is_centered_and_a_wide_table_reaches_into_the_margin;
mod an_empty_text_given_the_keyboard_takes_the_caret;
mod checkboxes_and_block_chips_line_up_with_their_text;
mod classifies_markdown_image;
mod clicking_into_the_text_shows_the_format_bar_without_a_keyboard_button;
mod code_is_painted_on_its_background;
mod foreign_organization_url_is_not_an_embed;
mod image_embed_directive_uses_markdown_image;
mod image_embed_directive_uses_plain_url;
mod inline_code_keeps_its_background_where_it_wraps;
mod lists_and_quotes_are_painted_with_bullets_and_bars;
mod markdown_is_painted_with_its_styles;
mod peers_carets_follow_their_presence_and_go_when_it_is_hidden;
mod pressing_the_format_bar_keeps_the_focus_and_the_selection_in_the_text;
mod quick_typing_undoes_a_word_at_a_time;
mod replacing_a_referenced_block_rewrites_its_url;
mod selecting_a_grapheme_keeps_prepend_with_ascii;
mod selecting_text_does_not_move_it;
mod shrinking_the_view_keeps_the_caret_in_sight_and_tells_the_input_method_where_it_is;
mod switching_to_hex_view_shows_the_bytes;
mod the_caret_is_shown_to_peers_in_a_color_they_are_not_using;
mod the_copy_button_copies_a_code_block_without_its_fences;
mod the_keyboard_reaches_the_format_bar_and_walks_it_with_the_arrows;
mod the_markdown_showcase_is_painted;
mod the_palette_and_the_shortcuts_run_the_format_actions;
mod the_text_settings_point_at_the_tab_whose_menu_opened_them;
mod the_text_settings_with_no_tab_showing_open_at_the_top_without_a_beak;
mod the_text_waits_for_its_content_and_typing_before_it_is_dropped;
mod typing_continues_at_the_caret_after_a_peer_edits_around_it;
mod typing_continues_where_a_peer_deleted_the_text_around_the_caret;
mod typing_follows_a_replacement_of_the_carets_byte;
mod typing_inserts_text_into_the_document;
mod up_and_down_step_through_the_rows_of_a_wrapped_paragraph;
mod up_and_down_step_through_the_rows_of_wrapped_code;
mod whitespace_markers_sit_over_real_spaces;

fn editor(text: &str) -> BeuiTest<TextRoot> {
    let block = Uuid::new_v4();
    let mut editor = BeuiTest::editing(block);
    editor.hold(None, TextBlock::of(text));
    editor.run();
    editor
}

fn text(editor: &BeuiTest<TextRoot>) -> String {
    TextBlock::text(&editor.content::<TextContent>(None))
}

const PEER: u64 = 2;

fn peer_insert(editor: &mut BeuiTest<TextRoot>, at: usize, typed: &str) {
    let edit = TextBlock::insert(
        &editor.content::<TextContent>(None),
        PEER,
        at,
        typed.as_bytes(),
    )
    .expect("the peer has somewhere to type");
    editor.edit::<TextContent>(None, &edit);
}

fn peer_delete(editor: &mut BeuiTest<TextRoot>, range: std::ops::Range<usize>) {
    let edit = TextBlock::delete(&editor.content::<TextContent>(None), range)
        .expect("the peer has something to delete");
    editor.edit::<TextContent>(None, &edit);
}

const BLOCK_ID: Uuid = Uuid::from_u128(0xe2b8_7b59_9c69_4d75_83fd_801b_2727_1388);
const ORGANIZATION_ID: Uuid = Uuid::from_u128(0x7a20_a314_e4aa_4ca7_b7ae_d68c_3249_0d9d);

fn caret_at(editor: &mut BeuiTest<TextRoot>, index: usize) {
    editor.click("text.surface");
    editor.run();
    editor.key(Key::Home, Modifiers::NONE);
    editor.run();
    for _ in 0..index {
        editor.key(Key::ArrowRight, Modifiers::NONE);
        editor.run();
    }
}

fn peer(client: u64, anchor: usize, focus: usize, color: PresenceColor) -> PeerPresence {
    let position = |index: usize| serde_json::json!({ "client": 0, "offset": index });
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

fn shown_cursor(editor: &mut BeuiTest<TextRoot>) -> Option<Option<serde_json::Value>> {
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

/// Presses `key` and returns where the caret then is, as shown to peers.
fn caret_after(editor: &mut BeuiTest<TextRoot>, key: Key) -> u64 {
    editor.key(key, Modifiers::NONE);
    editor.run();
    let cursor = shown_cursor(editor)
        .flatten()
        .expect("the editor shows its caret to its peers");
    cursor["focus"]["offset"]
        .as_u64()
        .expect("the caret has an offset")
}

/// Walks the caret from `start` down to the line after the wrapped line that
/// begins at `wrapped`, then back up, checking it crosses the wrapped line a row at
/// a time and comes back to where it began.
fn step_through_wrapped_line(
    editor: &mut BeuiTest<TextRoot>,
    start: usize,
    wrapped: std::ops::Range<usize>,
) {
    editor.presence_visible(true);
    editor.click("text.surface");
    editor.run();
    for _ in 0..wrapped.end {
        if caret_after(editor, Key::ArrowUp) == 0 {
            break;
        }
    }
    for _ in 0..start {
        editor.key(Key::ArrowRight, Modifiers::NONE);
        editor.run();
    }
    editor.record();
    let mut downs = Vec::new();
    while downs.last().is_none_or(|at| *at < wrapped.end as u64) && downs.len() < 20 {
        downs.push(caret_after(editor, Key::ArrowDown));
        editor.record();
    }
    let inside = downs
        .iter()
        .filter(|at| wrapped.contains(&(**at as usize)))
        .count();
    assert!(
        inside >= 3,
        "Down stops on each row of the wrapped line: {downs:?}"
    );
    assert!(
        downs.windows(2).all(|pair| pair[0] < pair[1]),
        "every Down moves on: {downs:?}"
    );
    let mut ups = Vec::new();
    for _ in 0..downs.len() {
        ups.push(caret_after(editor, Key::ArrowUp));
        editor.record();
    }
    let mut back = downs.clone();
    back.pop();
    back.reverse();
    back.push(start as u64);
    assert_eq!(ups, back, "Up retraces the rows Down took");
}
