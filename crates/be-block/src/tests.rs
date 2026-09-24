use super::*;
use crate::{
    browser_tab::{BrowserTabContent, HistoryItem},
    calendar::{Calendar, CalendarContent, CalendarEvent},
    checklist::{Checklist, ChecklistContent},
    counter::{Counter, CounterContent},
    database::{Database, DatabaseContent, DatabaseValue},
    database_schema::{
        DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale, DatabaseSchema,
        DatabaseSchemaContent,
    },
    database_view::{DatabaseView, DatabaseViewContent},
    image::{ImageContent, ImageHeader},
    text::{TextContent, TextLanguage, TextOp},
    ui_settings::{UiSettings, UiSettingsContent},
};

mod a_browser_tab_is_named_after_its_page;
mod a_browser_tab_push_discards_forward_history;
mod a_calendar_undo_keeps_what_someone_else_changed_since;
mod a_calendar_update_writes_only_the_fields_that_changed;
mod a_counter_reset_undoes_back_to_its_count;
mod a_database_and_its_views_reference_what_they_link_to;
mod a_database_grows_rows_to_fill_a_cell_and_drops_trailing_empty_ones;
mod a_logic_grid_edit_keeps_its_wires_normalized_and_follows_its_children;
mod a_map_keeps_its_points_in_bounds_and_follows_its_children;
mod a_presentation_moves_slides_by_index_and_follows_its_children;
mod a_review_forgets_or_repoints_an_approval_with_its_snapshot;
mod a_schema_normalizes_number_options_and_keeps_option_ids;
mod an_image_merges_only_when_one_side_changed_it;
mod calendars_merge_each_event_field_by_field;
mod canvas_children_are_removed_repointed_or_merged;
mod canvas_layers_reorder_and_keep_unlisted_slots;
mod canvases_merge_a_move_and_a_restyle_of_one_entity;
mod cells_set_on_both_sides_of_one_row_merge_to_both;
mod clearing_a_checklist_keeps_the_open_items;
mod compiled_logic_places_as_its_own_block_and_references_what_it_calls;
mod compiling_a_grid_derives_ports_from_its_inputs_and_outputs;
mod compiling_an_empty_grid_is_refused;
mod deleting_or_replacing_a_linked_block_rewrites_the_cells_that_link_it;
mod file_contents_check_what_they_hold_and_round_trip;
mod logic_game_solutions_keep_their_order_per_challenge;
mod logic_grids_merge_a_move_and_a_turn_of_one_component;
mod moves_played_on_both_sides_merge_to_both;
mod pixel_art_fills_resizes_and_keeps_a_paint_made_during_a_resize;
mod streamed_content_separates_its_header_from_its_payload;
mod text_merges_line_by_line_and_marks_real_conflicts;
mod text_operations_rebase_onto_concurrent_edits;
mod two_checklists_merge_every_item_either_side_added;
mod two_counters_merge_by_keeping_both_sides_of_the_count;
mod ui_settings_keep_the_zoom_in_bounds;
mod unpinning_a_component_removes_it_from_every_folder;
mod video_clips_attach_ripple_and_refuse_cycles;

fn header(name: &str) -> ImageHeader {
    ImageHeader {
        source_name: name.into(),
        media_type: "image/png".into(),
        width: 640,
        height: 480,
        failure: None,
    }
}

fn image(name: &str, payload: &[u8]) -> ImageContent {
    ImageContent::new(header(name), payload.to_vec())
}

fn applied(start: &str, operations: &[TextOp]) -> String {
    let mut content = TextContent::from(start);
    for operation in operations {
        content.apply(operation);
    }
    content.text()
}

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}

fn scheduled() -> (CalendarContent, crate::ObjectId) {
    let (id, add) = Calendar::add(&CalendarEvent::new("Standup", 540, 555));
    (edited(&CalendarContent::default(), [add]), id)
}
