use super::*;

pub(super) const DOCUMENT: &str = r#"{
    "id": "be3.counter",
    "name": "Counter",
    "version": "0.1.0",
    "editors": {
        "counter": {
            "id": "5b1f3c1e-2f7a-4d0e-9a51-0c6f2b8e7d41",
            "edits": "636f756e-7465-722d-626c-6f636b2d0001",
            "display_name": "Counter",
            "icon": "\ue0ef",
            "templates": {
                "main": {"category": "debug"},
                "zero": {"name": "Zero", "icon": "\ue167", "category": "template"},
                "another": {"block_type": "00007072-6573-656e-7461-74696f6e0001", "dialog": true}
            },
            "regions": ["Frame"]
        }
    },
    "entry_point": "counter.wasm"
}"#;

mod a_plugin_declares_its_block_types_and_their_module;
mod an_editor_is_found_by_its_id;
mod an_editor_that_edits_nothing_declares_no_templates;
mod an_editor_that_names_no_block_type_edits_nothing;
mod an_empty_entry_point_is_rejected;
mod manifest_document_defaults_what_the_host_can;
mod manifest_document_rejects_a_bad_block_type;
mod manifest_document_rejects_unknown_fields;
mod manifest_from_json_reads_a_document;
mod templates_keep_their_order_and_take_their_editors_defaults;
mod templates_of_an_editor_of_no_one_type_name_their_block_type;
mod two_editors_may_edit_one_block_type;
