use super::*;

pub(super) const DOCUMENT: &str = r#"{
    "id": "be3.counter",
    "name": "Counter",
    "version": "0.1.0",
    "editors": [
        {
            "block_type": "636f756e-7465-722d-626c-6f636b2d0001",
            "display_name": "Counter",
            "icon": "\ueb8d",
            "templates": {
                "main": {"category": "debug"},
                "zero": {"name": "Zero", "icon": "\ue3c6", "category": "template"},
                "another": {"block_type": "00007072-6573-656e-7461-74696f6e0001", "dialog": true}
            },
            "regions": ["Frame"],
            "chrome": ["Toolbar"]
        }
    ],
    "entry_point": "counter.wasm"
}"#;

mod an_empty_entry_point_is_rejected;
mod manifest_document_defaults_what_the_host_can;
mod manifest_document_rejects_a_bad_block_type;
mod manifest_document_rejects_unknown_fields;
mod manifest_from_json_reads_a_document;
mod templates_keep_their_order_and_take_their_editors_defaults;
