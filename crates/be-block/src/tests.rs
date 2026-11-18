use crate::display_settings::{DisplaySettings, DisplaySettingsContent};
use be_block_content::*;
use be_block_input::{InputSettings, InputSettingsContent};
use uuid::Uuid;

mod a_device_class_is_detected_and_brings_its_own_defaults;
mod a_device_uses_the_settings_profile_and_session_it_picked;
mod a_profile_layers_its_settings_over_the_defaults;
mod a_screen_back_at_its_default_mode_is_forgotten;
mod a_screen_turned_off_and_the_arrangement_are_kept_apart_from_its_mode;
mod offline_first_is_chosen_per_workspace_within_the_organizations_policy;
mod screens_turn_off_after_ten_minutes_unless_told_otherwise;
mod settings_resolve_from_the_organization_the_nearest_block_and_the_viewer;
mod settings_split_across_two_blocks_read_as_one;
mod visiting_a_block_puts_it_first_in_the_recents_once;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}
