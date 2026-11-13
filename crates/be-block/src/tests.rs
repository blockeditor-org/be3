use crate::{
    display_settings::{DisplaySettings, DisplaySettingsContent},
    input_settings::{InputSettings, InputSettingsContent},
};
use be_block_content::*;
use uuid::Uuid;

mod a_checkout_copy_answers_to_its_local_id_only_in_its_own_scope;
mod a_device_class_is_detected_and_brings_its_own_defaults;
mod a_device_uses_the_settings_profile_and_session_it_picked;
mod a_profile_layers_its_settings_over_the_defaults;
mod a_profile_picks_its_own_remapper_and_a_remapper_runs_once_it_names_a_module;
mod a_quoted_name_cannot_fake_what_a_dialog_says;
mod a_repository_keeps_its_latest_operations_and_knows_what_was_undone;
mod a_repository_moves_and_drops_branches;
mod a_screen_back_at_its_default_mode_is_forgotten;
mod a_screen_turned_off_and_the_arrangement_are_kept_apart_from_its_mode;
mod input_settings_keep_their_values_in_bounds;
mod offline_first_is_chosen_per_workspace_within_the_organizations_policy;
mod screens_turn_off_after_ten_minutes_unless_told_otherwise;
mod setting_every_pointer_replaces_what_each_device_overrode;
mod settings_resolve_from_the_organization_the_nearest_block_and_the_viewer;
mod settings_split_across_two_blocks_read_as_one;
mod two_search_indexes_merge_by_parts_into_one_finding_both;
mod visiting_a_block_puts_it_first_in_the_recents_once;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}
