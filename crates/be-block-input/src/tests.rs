use crate::{InputSettings, InputSettingsContent};
use be_block_content::*;
use uuid::Uuid;

mod a_profile_picks_its_own_remapper_and_a_remapper_runs_once_it_names_a_module;
mod input_settings_keep_their_values_in_bounds;
mod setting_every_pointer_replaces_what_each_device_overrode;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}
