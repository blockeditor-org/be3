use super::*;

const TYPES: &str = r#"{
    "id": "be3.checklist",
    "name": "Checklist",
    "version": "0.1.0",
    "block_types": {
        "checklist": {
            "id": "63686563-6b6c-6973-742d-626c6f636b31",
            "display_name": "Checklist",
            "icon": "",
            "color": [203, 91, 125],
            "children": {"add": true},
            "press": "Zoom",
            "module": "checklist_types.wasm"
        },
        "plain": {
            "id": "00000000-0000-4000-8000-000000000001",
            "display_name": "Plain",
            "icon": "x",
            "module": "checklist_types.wasm"
        }
    },
    "entry_point": "checklist.wasm"
}"#;

#[test]
fn a_plugin_declares_its_block_types_and_their_module() {
    let manifest = manifest_from_json(TYPES).expect("the manifest is valid");
    assert!(manifest.editors.is_empty());
    assert_eq!(manifest.type_module(), Some("checklist_types.wasm"));
    let [checklist, plain] = manifest.block_types.as_slice() else {
        panic!("the manifest declares two block types");
    };
    assert_eq!(checklist.name, "checklist");
    assert_eq!(
        Uuid::from_bytes(checklist.content_type()),
        Uuid::from_u128(0x6368_6563_6b6c_6973_742d_626c_6f63_6b31)
    );
    assert_eq!(checklist.descriptor.color, Some([203, 91, 125]));
    assert_eq!(
        checklist.descriptor.children,
        ChildOperations {
            add: true,
            delete: false,
            replace: false,
        }
    );
    assert_eq!(checklist.descriptor.press, Press::Zoom);
    assert_eq!(plain.descriptor.press, Press::Open);
    assert_eq!(plain.descriptor.color, None);
    assert_eq!(plain.module, "checklist_types.wasm");

    let two_modules = TYPES.replace(
        "\"icon\": \"x\",\n            \"module\": \"checklist_types.wasm\"",
        "\"icon\": \"x\",\n            \"module\": \"other.wasm\"",
    );
    assert_ne!(two_modules, TYPES);
    assert_eq!(
        manifest_from_json(&two_modules),
        Err(ManifestError::SeveralTypeModules)
    );
    let twice = TYPES.replace(
        "00000000-0000-4000-8000-000000000001",
        "63686563-6b6c-6973-742d-626c6f636b31",
    );
    assert_eq!(
        manifest_from_json(&twice),
        Err(ManifestError::DuplicateBlockType)
    );
    let without = TYPES.replace(
        ",\n            \"module\": \"checklist_types.wasm\"\n        }\n    },",
        "\n        }\n    },",
    );
    assert_ne!(without, TYPES);
    assert!(
        manifest_from_json(&without).is_err(),
        "a block type names the module that brings it"
    );
}
