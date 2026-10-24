use super::*;
use crate::canvas::{
    Canvas, CanvasContent, CanvasEntity, CanvasPoint, CanvasShape, CanvasTransform,
    InfiniteCanvasOperation,
};
use crate::{
    counter::{Counter, CounterContent},
    database::{Database, DatabaseContent, DatabaseValue},
    database_schema::{
        DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale, DatabaseSchema,
        DatabaseSchemaContent,
    },
    database_view::{DatabaseView, DatabaseViewContent},
    display_settings::{DisplaySettings, DisplaySettingsContent},
    input_settings::{InputSettings, InputSettingsContent},
    text::{TextBlock, TextContent, TextLanguage},
};
use be_model::ObjectId;
use uuid::Uuid;

mod a_block_link_is_found_by_its_fragment_whatever_its_host;
mod a_canvas_entity_removed_on_one_side_and_moved_on_the_other_stays_removed;
mod a_canvas_profile_is_kept_but_never_undone;
mod a_cell_set_while_someone_clears_the_last_one_in_its_row_is_kept;
mod a_checkout_copy_answers_to_its_local_id_only_in_its_own_scope;
mod a_component_added_to_one_entity_on_both_sides_is_kept_once;
mod a_counter_reset_undoes_back_to_its_count;
mod a_database_and_its_views_reference_what_they_link_to;
mod a_database_follows_the_pages_and_databases_its_rows_link_to;
mod a_database_grows_rows_to_fill_a_cell_and_drops_trailing_empty_ones;
mod a_database_is_described_by_its_prefix;
mod a_deleted_child_of_a_view_is_blanked_in_the_state_that_referenced_it;
mod a_device_class_is_detected_and_brings_its_own_defaults;
mod a_device_uses_the_settings_profile_and_session_it_picked;
mod a_language_changed_on_both_sides_counts_a_conflict;
mod a_presentation_moves_slides_by_index_and_follows_its_children;
mod a_profile_layers_its_settings_over_the_defaults;
mod a_profile_picks_its_own_remapper_and_a_remapper_runs_once_it_names_a_module;
mod a_repository_keeps_its_latest_operations_and_knows_what_was_undone;
mod a_repository_moves_and_drops_branches;
mod a_reset_racing_an_increment_keeps_the_increment;
mod a_row_is_found_by_its_short_id_with_or_without_its_prefix;
mod a_schema_field_removed_on_one_side_and_renamed_on_the_other_stays_removed;
mod a_schema_normalizes_number_options_and_keeps_option_ids;
mod a_screen_back_at_its_default_mode_is_forgotten;
mod a_view_hides_closed_choices_and_matches_any_of_several_values;
mod a_view_shows_only_the_rows_every_filter_matches;
mod an_arrow_end_bound_to_a_shape_follows_it_and_stays_when_it_is_removed;
mod an_icon_saved_as_a_material_codepoint_reads_as_its_lucide_name;
mod block_metadata_keeps_keys_it_does_not_know_and_reads_each_key_alone;
mod block_urls_include_organization_and_reject_malformed_paths;
mod canvas_children_are_removed_repointed_or_merged;
mod canvas_layers_reorder_and_keep_unlisted_slots;
mod canvases_merge_a_move_and_a_restyle_of_one_entity;
mod cells_set_in_a_new_row_by_two_peers_at_once_share_the_row;
mod cells_set_on_both_sides_of_one_row_merge_to_both;
mod clips_attached_to_each_other_on_each_side_merge_without_a_cycle;
mod content_is_stamped_with_the_shape_it_was_saved_in;
mod databases_saved_by_the_first_tracker_build_still_load;
mod defined_metadata_merges_key_by_key;
mod deleting_or_replacing_a_linked_block_rewrites_the_cells_that_link_it;
mod entities_brought_forward_on_both_sides_merge_to_both_moves;
mod enum_options_added_on_both_sides_merge_to_both;
mod frozen_database;
mod frozen_database_schema;
mod frozen_database_view;
mod frozen_text;
mod icon_overrides_round_trip_and_merge_independently;
mod input_settings_keep_their_values_in_bounds;
mod inserts_at_one_place_from_two_peers_keep_each_peers_text_whole;
mod links_added_on_both_sides_are_both_kept_and_show_on_the_rows_they_point_at;
mod moves_played_on_both_sides_merge_to_both;
mod screens_turn_off_after_ten_minutes_unless_told_otherwise;
mod setting_every_pointer_replaces_what_each_device_overrode;
mod settings_split_across_two_blocks_read_as_one;
mod slides_moved_by_two_peers_at_once_both_move;
mod slides_moved_on_both_sides_merge_to_both_moves;
mod streamed_content_separates_its_header_from_its_payload;
mod text_edited_in_different_paragraphs_merges_cleanly;
mod text_merges_line_by_line_and_marks_real_conflicts;
mod text_retyped_in_one_canvas_entity_by_two_peers_keeps_both;
mod text_saved_before_it_was_frozen_still_loads;
mod text_typed_inside_a_range_someone_deletes_at_once_survives;
mod the_frozen_blocks_read_every_register_without_any_one_field;
mod the_same_range_deleted_by_two_peers_at_once_is_deleted_once;
mod two_counters_merge_by_keeping_both_sides_of_the_count;
mod two_peers_restyling_different_fields_of_one_shape_keep_both;
mod undoing_a_reset_restores_only_what_was_reset;
mod unpinning_a_component_removes_it_from_every_folder;
mod video_clips_attach_ripple_and_refuse_cycles;
mod visiting_a_block_puts_it_first_in_the_recents_once;

#[derive(Clone, Debug, Default, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
struct Header {
    source_name: String,
    media_type: String,
}

struct Bytes;

impl crate::blob::BlobKind for Bytes {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6279_7465_732d_626c_6f63_6b2d_7465_7374);

    type Header = Header;

    fn name(header: &Header) -> &str {
        &header.source_name
    }
}

type BytesContent = crate::blob::Blob<Bytes>;

fn header(name: &str) -> Header {
    Header {
        source_name: name.into(),
        media_type: "image/png".into(),
    }
}

fn image(name: &str, payload: &[u8]) -> BytesContent {
    BytesContent::new(header(name), payload.to_vec())
}

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
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

fn rectangle() -> CanvasEntity {
    CanvasEntity::new(
        CanvasShape::Rectangle,
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
    )
}

fn canvas_run(content: &CanvasContent, operation: InfiniteCanvasOperation) -> CanvasContent {
    edited(content, [Canvas::edit_in(content, &operation)])
}
