//! The type-module world generated again with nothing mapped but its ids (`uuid`, `hash`,
//! `commit-id`, mapped as the crate maps them), beside the crate's own, whose `with:` puts the
//! block stack's types in place of the WIT's: each pair must have one wire form.

use std::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};

mod generated {
    #![allow(dead_code, unused_imports)]

    be_wasm_ops_bindgen::generate!({
        path: "wit",
        world: "type-module",
        deps: ["../be-wasm-ops/wit"],
        root: crate::tests::generated,
        with: {
            "uuid": uuid::Uuid,
            "hash": be_block_store::Hash,
            "commit-id": be_block_commit::CommitId,
        },
        copy: ["uuid", "hash", "commit-id"],
        default: ["plain", "description", "plain-file", "status"],
        inline: ["replace-child", "reorder-child"],
    });
}

pub(crate) use generated::types as wit;

/// `value`, encoded as the crate's mapped type `R`, decodes as `G`, the type the WIT describes,
/// and back to the same bytes and the same value; with `fields`, both also read field for field
/// alike (their `Debug`, which names each field), for a pair whose Rust and WIT names agree.
pub(crate) fn agrees<R, G>(value: R, fields: bool)
where
    R: Serialize + DeserializeOwned + PartialEq + Debug,
    G: Serialize + DeserializeOwned + Debug,
{
    let bytes = crate::ops::encode(&value);
    let generated: G = crate::ops::decode(&bytes)
        .unwrap_or_else(|| panic!("{value:?} does not decode as the WIT's {}", short::<G>()));
    let again = crate::ops::encode(&generated);
    assert_eq!(
        again,
        bytes,
        "{value:?} encodes otherwise as the WIT's {}: {generated:?}",
        short::<G>()
    );
    let back: R = crate::ops::decode(&again).expect("the same bytes decode again");
    assert_eq!(back, value);
    if fields {
        assert_eq!(format!("{generated:?}"), format!("{value:?}"));
    }
}

fn short<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or_default()
}

mod every_type_the_wit_maps_onto_the_stacks_has_its_wire_form;
