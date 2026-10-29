use super::*;
use crate::{
    display_settings::{DisplaySettings, DisplaySettingsContent},
    input_settings::{InputSettings, InputSettingsContent},
};
use uuid::Uuid;

mod a_block_link_is_found_by_its_fragment_whatever_its_host;
mod a_checkout_copy_answers_to_its_local_id_only_in_its_own_scope;
mod a_deleted_child_of_a_view_is_blanked_in_the_state_that_referenced_it;
mod a_device_class_is_detected_and_brings_its_own_defaults;
mod a_device_uses_the_settings_profile_and_session_it_picked;
mod a_profile_layers_its_settings_over_the_defaults;
mod a_profile_picks_its_own_remapper_and_a_remapper_runs_once_it_names_a_module;
mod a_repository_keeps_its_latest_operations_and_knows_what_was_undone;
mod a_repository_moves_and_drops_branches;
mod a_screen_back_at_its_default_mode_is_forgotten;
mod an_icon_saved_as_a_material_codepoint_reads_as_its_lucide_name;
mod block_metadata_keeps_keys_it_does_not_know_and_reads_each_key_alone;
mod block_urls_include_organization_and_reject_malformed_paths;
mod defined_metadata_merges_key_by_key;
mod icon_overrides_round_trip_and_merge_independently;
mod input_settings_keep_their_values_in_bounds;
mod metadata_that_points_to_a_block_lists_it_as_a_reference;
mod offline_first_is_chosen_per_workspace_within_the_organizations_policy;
mod screens_turn_off_after_ten_minutes_unless_told_otherwise;
mod setting_every_pointer_replaces_what_each_device_overrode;
mod settings_resolve_from_the_organization_the_nearest_block_and_the_viewer;
mod settings_split_across_two_blocks_read_as_one;
mod streamed_content_separates_its_header_from_its_payload;
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
