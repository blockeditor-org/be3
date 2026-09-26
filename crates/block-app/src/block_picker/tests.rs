use std::collections::HashSet;
use std::sync::Arc;

use block_plugin_api::manifest_from_json;
use uuid::Uuid;

use super::{
    CreateView, PickerView, add_sections, creation_surface, publish, template_sections, views,
};
use crate::editors::EditorRegistry;
use crate::surfaces::SurfaceId;

mod a_picker_opened_from_a_creation_dialog_is_shown_above_it;
mod a_picker_with_nothing_to_show_is_not_shown;
mod important_regular_and_debug_blocks_sit_under_their_own_headings;
mod templates_are_grouped_under_the_editor_that_declares_them;

const DECK: &str = "00007072-6573-656e-7461-74696f6e0001";
const CANVAS: &str = "696e6669-6e69-7465-2d63-616e76617301";

const PLUGINS: [&str; 2] = [
    r#"{
        "id": "be3.test-deck",
        "name": "Deck",
        "version": "1",
        "editors": [{
            "block_type": "00007072-6573-656e-7461-74696f6e0001",
            "display_name": "Presentation",
            "icon": "a",
            "templates": {
                "main": {},
                "title-slide": {"name": "Title page", "icon": "t", "category": "template", "block_type": "696e6669-6e69-7465-2d63-616e76617301"},
                "blank-slide": {"name": "Blank page", "icon": "b", "category": "template", "block_type": "696e6669-6e69-7465-2d63-616e76617301"}
            },
            "regions": ["Frame"]
        }],
        "entry_point": "deck.wasm"
    }"#,
    r#"{
        "id": "be3.test-tools",
        "name": "Tools",
        "version": "1",
        "editors": [
            {
                "block_type": "696e6669-6e69-7465-2d63-616e76617301",
                "display_name": "Canvas",
                "icon": "c",
                "templates": {"main": {"category": "important"}},
                "regions": ["Frame"]
            },
            {
                "block_type": "636f756e-7465-722d-626c-6f636b2d0001",
                "display_name": "Counter",
                "icon": "n",
                "templates": {"main": {"category": "debug"}},
                "regions": ["Frame"]
            },
            {
                "block_type": "66696c65-2d74-7265-652d-626c6f636b01",
                "display_name": "Files",
                "icon": "f",
                "regions": ["Frame"]
            }
        ],
        "entry_point": "tools.wasm"
    }"#,
];

fn registry() -> EditorRegistry {
    EditorRegistry::from_manifests(
        PLUGINS
            .iter()
            .map(|plugin| Arc::new(manifest_from_json(plugin).expect("the manifest is valid")))
            .collect(),
    )
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("a uuid")
}

fn creating(id: Uuid, depth: usize) -> PickerView {
    PickerView {
        id,
        depth,
        choose: None,
        create: Some(CreateView {
            title: "Game".to_owned(),
            working: false,
            ready: false,
            dialog: true,
        }),
        error: None,
    }
}
