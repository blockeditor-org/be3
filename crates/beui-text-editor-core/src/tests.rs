use std::sync::Arc;

use super::*;

mod a_long_fenced_block_stays_code_wherever_it_is_read;
mod atomic_units_are_single_cursor_units;
mod changes_since_coalesce_the_edits_after_a_revision;
mod collapse_and_uncollapse_affect_touched_lines;
mod collapsed_section_reveals_temporarily_around_the_cursor;
mod collapsible_sections_detects_indentation_blocks;
mod collapsible_sections_detects_markdown_headings;
mod core;
mod ctrl_d;
mod double_click_selects_the_word_beside_the_caret_within_its_line;
mod down_and_up_arrows_step_through_a_fenced_code_block;
mod down_arrow_skips_past_a_collapsed_section;
mod dragging_back_after_selecting_past_the_edge_follows_the_pointer;
mod find_matches_is_case_insensitive_by_default;
mod find_matches_matches_are_non_overlapping;
mod find_matches_respects_case_sensitive_flag;
mod find_matches_returns_empty_for_empty_query;
mod find_next_cycles_through_matches_and_wraps;
mod find_previous_cycles_through_matches_and_wraps;
mod find_status_reports_current_match_index;
mod has_stop;
mod ime_a_composing_region_over_existing_text_is_recomposed;
mod ime_any_other_command_ends_the_composition;
mod ime_commit_replaces_the_composition_and_ends_it;
mod ime_composing_text_is_written_into_the_document_and_replaced_as_it_changes;
mod ime_composition_excludes_text_inserted_after_its_last_byte;
mod ime_delete_surrounding_removes_text_beside_the_selection;
mod ime_replace_text_swaps_a_typed_word_for_a_suggestion;
mod ime_state_reports_the_text_around_the_selection;
mod indentation_width_is_configured_through_operations;
mod markdown_block_styles;
mod markdown_checkbox_toggle;
mod markdown_formatting;
mod markdown_formatting_toggles_off;
mod markdown_incremental_edit;
mod markdown_inline_code_escapes_backtick_runs;
mod markdown_inline_code_pads_backtick_edges;
mod markdown_inline_code_toggle_off;
mod markdown_inline_styles;
mod markdown_invalid_utf8;
mod markdown_lines_are_described_from_the_parse_tree;
mod markdown_list_newline;
mod markdown_plain_punctuation;
mod markdown_styles_hold_across_style_windows;
mod markdown_windows_follow_edits_inside_them;
mod positions_follow_edits_and_come_back_with_undo;
mod positions_follow_their_byte_through_replacement_and_undo;
mod positions_in_a_large_buffer_resolve_back_to_their_index;
mod raw_bytes;
mod replace_all_matches_does_nothing_when_there_are_no_matches;
mod replace_all_matches_replaces_every_match;
mod replace_match_does_nothing_when_selection_is_not_on_a_match;
mod replace_match_replaces_current_and_advances_to_next_match;
mod replace_ranges_replaces_every_range;
mod rich_text_a_code_block_is_decorated_by_its_language;
mod rich_text_a_heading_covers_its_line_and_enter_at_its_end_starts_a_body_line;
mod rich_text_a_link_does_not_grow_as_text_is_typed_at_its_end;
mod rich_text_a_style_toggled_at_the_caret_applies_to_what_is_typed_next;
mod rich_text_a_style_toggled_at_the_caret_carries_across_enter;
mod rich_text_a_transform_composes_generic_operations;
mod rich_text_a_void_is_one_step_for_the_caret_and_carries_its_props;
mod rich_text_backspace_at_a_block_start_lifts_its_props;
mod rich_text_backspace_inside_a_code_block_joins_its_lines;
mod rich_text_bold_toggles_over_a_selection;
mod rich_text_enter_at_the_start_of_a_checked_item_keeps_it_checked;
mod rich_text_enter_continues_a_list_and_lifts_out_of_an_empty_item;
mod rich_text_enter_on_a_second_blank_line_ends_a_code_block;
mod rich_text_enter_on_an_empty_nested_item_outdents_it;
mod rich_text_formats_kept_across_edits_match_formats_resolved_afresh;
mod rich_text_joining_lines_keeps_the_first_lines_block;
mod rich_text_marks_converge_in_any_order;
mod rich_text_moving_past_a_styled_last_line_opens_a_plain_line;
mod rich_text_tab_in_a_code_block_indents_its_text;
mod rich_text_tab_indents_a_list_item;
mod rich_text_typing_continues_the_format_before_the_caret;
mod rich_text_undo_and_redo_bring_formatting_back;
mod rich_text_undo_and_redo_restore_every_random_edit;
mod rich_text_undoing_a_join_brings_back_both_lines_blocks;
mod right_arrow_opens_a_collapsed_line;
mod rust_syn_hl;
mod select_up_after_moving_down_starts_from_the_current_column;
mod shift_click_extends_a_keyboard_selection_from_its_anchor;
mod slices_are_stitched_together_from_chunks;
mod styles_read_before_an_edit_are_redone_around_it;
mod tabs_are_used_for_automatic_indentation;
mod toggle_collapse_at_is_independent_of_the_cursor;
mod toggle_collapse_at_leaves_a_cursor_outside_the_section_untouched;
mod toggle_collapse_at_moves_the_cursor_out_of_a_newly_hidden_section;
mod toml_syn_hl;
mod up_arrow_skips_past_a_collapsed_section;
mod virtual_end_anchors_follow_edits_without_adding_a_byte;
mod zig_syn_hl;

fn rendered_highlight(tester: &mut EditorTester, offset: usize) -> String {
    let highlight = tester.editor.highlight();
    let read = tester.editor.document().read().unwrap();
    let document = DocumentView::new(&*read);
    let mut result = String::new();
    let mut previous = SynHlColorScope::Invalid;
    for index in offset..document.len() {
        let scope = highlight.advance_and_read(index);
        let byte = document.bytes()[index];
        if !byte.is_ascii_whitespace() && scope != previous {
            result.push('<');
            result.push_str(scope.as_str());
            result.push('>');
        }
        previous = scope;
        result.push(char::from(byte));
    }
    result
}

fn formatted(tester: &EditorTester) -> String {
    let read = tester.editor.document().read().unwrap();
    let formats = read.formats().expect("a rich text document has formats");
    let bytes = DocumentView::new(&*read).bytes().to_vec();
    let mut result = String::new();
    let mut previous = MarkSet::default();
    for (index, byte) in bytes.iter().enumerate() {
        let block = formats.block_at(index).unwrap();
        if index == block.start && !block.props.is_empty() {
            result.push_str(&format!("[{}]", tag(&block.props)));
        }
        let marks = formats.marks_at(index).unwrap();
        if *marks != previous {
            result.push_str(&format!("{{{}}}", tag(marks)));
            previous = marks.clone();
        }
        result.push(char::from(*byte));
    }
    result
}

fn tag(marks: &MarkSet) -> String {
    marks
        .iter()
        .map(|(key, value)| match value {
            Value::Bool(true) => key.to_string(),
            Value::Bool(false) => format!("!{key}"),
            Value::Int(value) => format!("{key}={value}"),
            Value::Text(value) => format!("{key}={value}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn select(tester: &mut EditorTester, range: std::ops::Range<usize>) {
    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(range.start),
        focus: tester.pos(range.end),
    });
}

fn tool(tester: &mut EditorTester, name: &str) {
    let schema = tester.editor.schema();
    let tool = schema
        .tools()
        .iter()
        .find(|tool| tool.name == name)
        .copied()
        .expect("the schema has the tool");
    let selected = tester.editor.selected().unwrap();
    tester.execute((tool.command)(&selected));
}

fn active(tester: &EditorTester, name: &str) -> bool {
    let schema = tester.editor.schema();
    let tool = schema
        .tools()
        .iter()
        .find(|tool| tool.name == name)
        .unwrap();
    (tool.active)(&tester.editor.selected().unwrap())
}

struct EditorTester {
    editor: Core,
}

impl EditorTester {
    fn new(initial: impl AsRef<[u8]>) -> Self {
        Self::with_language(initial, TextLanguage::Zig)
    }

    fn with_language(initial: impl AsRef<[u8]>, language: TextLanguage) -> Self {
        let mut editor = Core::new(Arc::new(TextBuffer::new(initial)));
        editor.set_language(language);
        Self { editor }
    }

    fn rich(initial: impl AsRef<[u8]>) -> Self {
        let mut editor = Core::new(Arc::new(TextBuffer::new(initial)));
        editor.set_language(TextLanguage::RichText);
        editor.set_schema(Arc::new(schema::basic::Basic));
        let start = editor.position(0);
        editor.select(Selection::at(start));
        Self { editor }
    }

    fn bytes(&self) -> Vec<u8> {
        let read = self.editor.document().read().unwrap();
        DocumentView::new(&*read).bytes().to_vec()
    }

    fn pos(&self, byte: usize) -> Position {
        self.editor.position(byte)
    }

    fn execute(&mut self, command: EditorCommand<'_>) {
        self.editor.execute_command(command);
    }

    fn set_cursor(&mut self, position: Position) {
        self.execute(EditorCommand::SetSelection {
            anchor: position,
            focus: position,
        });
    }

    fn expect_content(&self, expected: impl AsRef<[u8]>) {
        let read = self.editor.document().read().unwrap();
        let document = DocumentView::new(&*read);
        let mut markers = Vec::new();
        for cursor in self.editor.cursor_positions() {
            let anchor = cursor.pos.anchor.resolve(&document);
            let focus = cursor.pos.focus.resolve(&document);
            if anchor == focus {
                markers.push((focus, b'|'));
            } else if anchor < focus {
                markers.push((anchor, b'['));
                markers.push((focus, b'|'));
            } else {
                markers.push((focus, b'|'));
                markers.push((anchor, b']'));
            }
        }
        markers.sort_by_key(|(index, _)| *index);
        let mut actual = Vec::new();
        for index in 0..=document.len() {
            actual.extend(
                markers
                    .iter()
                    .filter(|(marker_index, _)| *marker_index == index)
                    .map(|(_, marker)| *marker),
            );
            if let Some(byte) = document.bytes().get(index) {
                actual.push(*byte);
            }
        }
        assert_eq!(
            actual,
            expected.as_ref(),
            "cursors: {:?}",
            self.editor.cursor_positions()
        );
    }
}

fn backspace() -> EditorCommand<'static> {
    EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    }
}
