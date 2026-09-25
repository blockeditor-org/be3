use std::collections::HashSet;
use std::sync::Arc;

use block_plugin_api::manifest_from_json;
use uuid::Uuid;

use super::{add_sections, template_sections};
use crate::editors::EditorRegistry;

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
