use be_block::{
    BlockContent, BlockMetadata, ChildChange, LiveEdit, Merge, Root, derived_metadata, metadata,
};
use be_commit::MergeResult;
use be_model::ObjectId;
use uuid::Uuid;

use crate::database::{Database, DatabaseContent, DatabaseValue};
use crate::database_schema::{
    DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale, DatabaseSchema,
    DatabaseSchemaContent,
};
use crate::database_view::{DatabaseView, DatabaseViewContent};

mod a_cell_set_while_someone_clears_the_last_one_in_its_row_is_kept;
mod a_database_and_its_views_reference_what_they_link_to;
mod a_database_follows_the_pages_and_databases_its_rows_link_to;
mod a_database_grows_rows_to_fill_a_cell_and_drops_trailing_empty_ones;
mod a_database_is_described_by_its_prefix;
mod a_row_is_found_by_its_short_id_with_or_without_its_prefix;
mod a_schema_field_removed_on_one_side_and_renamed_on_the_other_stays_removed;
mod a_schema_normalizes_number_options_and_keeps_option_ids;
mod a_view_hides_closed_choices_and_matches_any_of_several_values;
mod a_view_shows_only_the_rows_every_filter_matches;
mod cells_set_in_a_new_row_by_two_peers_at_once_share_the_row;
mod cells_set_on_both_sides_of_one_row_merge_to_both;
mod databases_read_every_register_without_any_one_field;
mod databases_saved_by_the_first_tracker_build_still_load;
mod deleting_or_replacing_a_linked_block_rewrites_the_cells_that_link_it;
mod enum_options_added_on_both_sides_merge_to_both;
mod frozen_database;
mod frozen_database_schema;
mod frozen_database_view;
mod links_added_on_both_sides_are_both_kept_and_show_on_the_rows_they_point_at;

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
