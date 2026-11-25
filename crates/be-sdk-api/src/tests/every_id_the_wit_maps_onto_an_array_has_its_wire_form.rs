//! The apps world's blocks interface generated again with its `uuid` and `hash` unmapped
//! (`test-wit`), beside the crate's own,
//! which keeps them as `[u8; 16]` and `[u8; 32]` and encodes them as the WIT's `list<u8>`
//! (`encodings`): a record holding them must have one wire form either way.

use std::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};

mod generated {
    #![allow(dead_code, unused_imports, clippy::all)]

    be_wasm_ops_bindgen::generate!({
        path: "test-wit",
        world: "wire",
        deps: ["../be-lib-ui-keys/wit", "../be-lib-ui-preferences/wit", "../be-lib-ui-types/wit", "../be-wasm-ops/wit", "../be-wasm-gpu-abi/wit", "../be-sdk-manifest/wit", "wit"],
        root: crate::tests::every_id_the_wit_maps_onto_an_array_has_its_wire_form::generated,
        uses: {
            "be3:manifest/types@0.1.0": ::be_sdk_manifest::types,
            "be3:ui/types@0.1.0": ::be_lib_ui_types::types,
            "be3:preferences/types@0.1.0": ::be_lib_ui_preferences::types,
            "be3:keys/types@0.1.0": ::be_lib_ui_keys::types,
        },
    });
}

/// `value`, encoded as the crate's type `R`, decodes as `G`, the type the WIT describes, and
/// back to the same bytes and the same value, reading field for field alike (a `[u8; N]` and a
/// `Vec<u8>` print the same).
fn agrees<R, G>(value: R)
where
    R: Serialize + DeserializeOwned + PartialEq + Debug,
    G: Serialize + DeserializeOwned + Debug,
{
    let bytes = crate::ops::encode(&value);
    let generated: G = crate::ops::decode(&bytes)
        .unwrap_or_else(|| panic!("{value:?} does not decode as the WIT describes it"));
    let again = crate::ops::encode(&generated);
    assert_eq!(again, bytes, "{value:?} encodes otherwise as the WIT's: {generated:?}");
    let back: R = crate::ops::decode(&again).expect("the same bytes decode again");
    assert_eq!(back, value);
    assert_eq!(format!("{generated:?}"), format!("{value:?}"));
}

#[test]
fn every_id_the_wit_maps_onto_an_array_has_its_wire_form() {
    agrees::<_, generated::app::blocks::SearchHit>(crate::SearchHit {
        block_id: [1; 16],
        key: "row".to_owned(),
        snippet: "a match".to_owned(),
        matches: vec![(2, 7)],
    });
    agrees::<_, generated::app::blocks::BlockHistory>(crate::blocks::BlockHistory {
        head: Some([3; 32]),
        can_undo: true,
        can_redo: false,
        restarted_at: None,
        lost_edits: false,
    });
}
