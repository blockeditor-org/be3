use super::*;
use crate::canvas::{
    CanvasContent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
    InfiniteCanvasOperation,
};
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
    logic_game::LogicGameContent,
    logic_grid::{LogicGridContent, LogicGridOperation},
    text::{TextContent, TextLanguage, TextOp},
    ui_settings::{UiSettings, UiSettingsContent},
};
use be_model::ObjectId;
use logicgame::grid::{
    Component, ComponentId, ComponentKind, ComponentOrientation, Point, Scale, Wire,
};
use uuid::Uuid;

mod a_block_added_to_a_folder_by_both_sides_is_listed_once;
mod a_browser_tab_is_named_after_its_page;
mod a_browser_tab_push_discards_forward_history;
mod a_calendar_undo_keeps_what_someone_else_changed_since;
mod a_calendar_update_writes_only_the_fields_that_changed;
mod a_canvas_entity_removed_on_one_side_and_moved_on_the_other_is_kept;
mod a_cell_set_while_someone_clears_the_last_one_in_its_row_is_kept;
mod a_component_added_to_one_entity_on_both_sides_is_kept_once;
mod a_component_moved_on_both_sides_conflicts_and_keeps_ours;
mod a_component_removed_on_one_side_and_turned_on_the_other_is_kept;
mod a_counter_reset_undoes_back_to_its_count;
mod a_database_and_its_views_reference_what_they_link_to;
mod a_database_grows_rows_to_fill_a_cell_and_drops_trailing_empty_ones;
mod a_folder_lists_each_block_once_and_follows_its_children;
mod a_language_changed_on_both_sides_counts_a_conflict;
mod a_logic_grid_edit_keeps_its_wires_normalized_and_follows_its_children;
mod a_map_keeps_its_points_in_bounds_and_follows_its_children;
mod a_pixel_painted_outside_the_other_sides_crop_counts_as_a_conflict;
mod a_presentation_moves_slides_by_index_and_follows_its_children;
mod a_reset_racing_an_increment_keeps_the_increment;
mod a_review_forgets_or_repoints_an_approval_with_its_snapshot;
mod a_schema_field_removed_on_one_side_and_renamed_on_the_other_is_kept;
mod a_schema_normalizes_number_options_and_keeps_option_ids;
mod a_wire_cut_on_one_side_stays_cut_when_the_other_extends_it;
mod a_wire_cut_stays_cut_when_someone_extends_it_at_once;
mod a_wire_removed_on_one_side_stays_removed_when_the_other_branches_off_it;
mod an_event_rescheduled_on_both_sides_conflicts_and_keeps_ours;
mod an_image_merges_only_when_one_side_changed_it;
mod an_update_for_an_event_someone_removed_does_not_bring_it_back;
mod block_urls_include_workspace_and_reject_malformed_paths;
mod calendars_merge_each_event_field_by_field;
mod canvas_children_are_removed_repointed_or_merged;
mod canvas_layers_reorder_and_keep_unlisted_slots;
mod canvases_merge_a_move_and_a_restyle_of_one_entity;
mod cells_set_in_a_new_row_by_two_peers_at_once_share_the_row;
mod cells_set_on_both_sides_of_one_row_merge_to_both;
mod checks_of_different_items_at_once_both_stick;
mod clearing_a_checklist_keeps_the_open_items;
mod clearing_done_items_keeps_one_someone_reopened_at_once;
mod clearing_done_items_while_the_other_side_renames_one_keeps_it;
mod clips_attached_to_each_other_on_each_side_merge_without_a_cycle;
mod compiled_logic_places_as_its_own_block_and_references_what_it_calls;
mod compiling_a_grid_derives_ports_from_its_inputs_and_outputs;
mod compiling_an_empty_grid_is_refused;
mod components_added_by_two_peers_at_once_are_both_kept;
mod components_added_on_both_sides_offline_are_both_kept;
mod deleting_or_replacing_a_linked_block_rewrites_the_cells_that_link_it;
mod entities_brought_forward_on_both_sides_merge_to_both_moves;
mod enum_options_added_on_both_sides_merge_to_both;
mod file_contents_check_what_they_hold_and_round_trip;
mod inserts_at_one_place_from_two_peers_keep_each_peers_text_whole;
mod items_added_by_two_peers_at_once_are_both_kept;
mod logic_game_solutions_keep_their_order_per_challenge;
mod logic_grids_merge_a_move_and_a_turn_of_one_component;
mod moves_played_on_both_sides_merge_to_both;
mod pages_opened_on_both_sides_are_both_kept_in_history;
mod pixel_art_fills_resizes_and_keeps_a_paint_made_during_a_resize;
mod resetting_a_ray_traced_scene_clears_pixels_and_entities;
mod settings_resolve_per_client_and_follow_their_children;
mod slides_moved_by_two_peers_at_once_both_move;
mod slides_moved_on_both_sides_merge_to_both_moves;
mod solutions_and_quiz_answers_made_on_both_sides_merge_to_both;
mod streamed_content_separates_its_header_from_its_payload;
mod text_edited_in_different_paragraphs_merges_cleanly;
mod text_merges_line_by_line_and_marks_real_conflicts;
mod text_operations_rebase_onto_concurrent_edits;
mod text_typed_inside_a_range_someone_deletes_at_once_survives;
mod the_same_range_deleted_by_two_peers_at_once_is_deleted_once;
mod the_same_solution_inserted_by_two_peers_at_once_is_listed_once;
mod the_same_wire_drawn_on_both_sides_merges_to_one;
mod two_checklists_merge_every_item_either_side_added;
mod two_counters_merge_by_keeping_both_sides_of_the_count;
mod ui_settings_keep_the_zoom_in_bounds;
mod undoing_a_clear_keeps_a_pixel_painted_since;
mod undoing_a_clear_puts_the_items_back_in_order;
mod undoing_a_component_removal_brings_it_back_with_its_wires_untouched;
mod undoing_a_reset_restores_only_what_was_reset;
mod undoing_a_wire_cut_joins_the_wire_again;
mod undoing_a_wire_keeps_the_part_someone_else_drew_onto_it;
mod undoing_an_event_removal_restores_it_with_its_id;
mod unpinning_a_component_removes_it_from_every_folder;
mod video_clips_attach_ripple_and_refuse_cycles;
mod wires_drawn_on_each_side_that_meet_join_into_one;

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

fn merged<C: Merge>(base: &C, ours: &C, theirs: &C) -> (C, usize) {
    match C::merge3(base, ours, theirs) {
        MergeResult::Clean(value) => (value, 0),
        MergeResult::Conflicted { value, conflicts } => (value, conflicts),
    }
}

fn undone<C: Undo>(content: &mut C, operation: C::Op) -> C::Step {
    let step = content
        .step(&operation)
        .expect("the operation changes something");
    content.apply(&operation);
    step
}

fn reverted<C: Undo>(content: &mut C, step: &C::Step) {
    for operation in content.revert(step) {
        content.apply(&operation);
    }
}

fn logic(content: &LogicGridContent, operations: &[LogicGridOperation]) -> Edit {
    content.root().edit_for_all(operations)
}

fn logic_run(content: &LogicGridContent, operations: &[LogicGridOperation]) -> LogicGridContent {
    edited(content, [logic(content, operations)])
}

fn logic_wire(start: (i64, i64), end: (i64, i64)) -> Wire {
    Wire::new(
        Point::new(start.0, start.1),
        Point::new(end.0, end.1),
        Scale::ONE,
    )
    .expect("a straight wire at least one cell long")
}

fn not_gate(id: ComponentId, x: i64, y: i64) -> Component {
    Component {
        id,
        position: Point::new(x, y),
        orientation: ComponentOrientation::Up,
        kind: ComponentKind::Not { scale: Scale::ONE },
    }
}

fn rectangle() -> CanvasEntity {
    CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}

fn canvas_run(content: &CanvasContent, operation: InfiniteCanvasOperation) -> CanvasContent {
    edited(content, [content.root().edit_for(&operation)])
}
