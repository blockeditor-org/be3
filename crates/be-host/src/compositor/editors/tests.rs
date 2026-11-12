use super::*;
use std::sync::Arc;
mod a_block_placed_twice_has_an_editor_in_each_place;
mod a_child_asking_for_another_editor_reopens_in_it;
mod a_conflicted_block_offers_both_versions;
mod a_window_child_placed_hidden_is_heard_but_not_drawn;
mod a_window_childs_status_is_its_programs;
mod app_editors_close_with_their_owner_rather_than_when_the_thread_ends;
mod escape_stops_a_presentation_rather_than_reaching_the_editor;

use be_sdk_api::{AppIdentity, Edits};
use be_sdk_manifest::{AppManifest, EditorManifest};

pub(super) fn editor(name: &str, edits: Edits) -> EditorManifest {
    EditorManifest {
        id: Uuid::new_v4().into_bytes(),
        name: name.to_owned(),
        edits,
        display_name: name.to_owned(),
        icon: String::new(),
        templates: Vec::new(),
        interaction: Default::default(),
        capabilities: Default::default(),
        resize: Default::default(),
        views: Vec::new(),
        accepts: Vec::new(),
        opens: Vec::new(),
    }
}

pub(super) fn app(id: &str, editors: Vec<EditorManifest>) -> Arc<AppManifest> {
    Arc::new(AppManifest {
        identity: AppIdentity {
            id: id.to_owned(),
            name: id.to_owned(),
            version: "1".to_owned(),
        },
        editors,
        block_types: Vec::new(),
        entry_point: String::new(),
        network: Vec::new(),
        extensions: Vec::new(),
        block_agents: false,
    })
}
