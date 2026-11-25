//! The protocol world generated again with nothing mapped but its ids (`uuid`, `hash`,
//! `commit-id`, `public-key`, `entry-hash`, as the crate maps them), beside the crate's own, whose `with:` puts the block
//! stack's types in place of the WIT's: each pair must have one wire form.

use std::fmt::Debug;

use be_block_graph::{Access, BlockParent};
use serde::{Serialize, de::DeserializeOwned};

use super::*;

mod generated {
    #![allow(dead_code, unused_imports)]

    be_wasm_ops_bindgen::generate!({
        path: "wit",
        world: "client",
        deps: ["../be-wasm-ops/wit"],
        root: crate::tests::every_type_the_wit_maps_has_its_wire_form::generated,
        network: true,
        with: {
            "uuid": uuid::Uuid,
            "hash": be_block_store::Hash,
            "commit-id": be_block_commit::CommitId,
            "public-key": [u8; 32],
            "entry-hash": [u8; 32],
        },
        copy: ["uuid", "hash", "commit-id", "public-key", "entry-hash", "access-level", "block-parent"],
        default: [
            "key-set",
            "key-work",
            "session-state",
            "access-list",
            "organization-kind",
            "object-change",
            "reference-change",
        ],
    });
}

/// `value`, encoded as the crate's mapped type `R`, decodes as `G`, the type the WIT describes,
/// and back to the same bytes and the same value.
fn agrees<R, G>(value: R)
where
    R: Serialize + DeserializeOwned + PartialEq + Debug,
    G: Serialize + DeserializeOwned + Debug,
{
    let bytes = postcard::to_stdvec(&value).expect("it encodes");
    let generated: G = postcard::from_bytes(&bytes)
        .unwrap_or_else(|_| panic!("{value:?} does not decode as the WIT describes it"));
    let again = postcard::to_stdvec(&generated).expect("it encodes");
    assert_eq!(again, bytes, "{value:?} encodes otherwise as the WIT's: {generated:?}");
    let back: R = postcard::from_bytes(&again).expect("the same bytes decode again");
    assert_eq!(back, value);
}

/// Each type `with:` maps holds the shape of the WIT type it stands for, so the WIT still
/// describes the wire: a value of it decodes as the type the generator would make, and back.
#[test]
fn every_type_the_wit_maps_has_its_wire_form() {
    // The WIT's ids are a `list<u8>` of their bytes, length first (a `Vec<u8>` encodes as the
    // list does).
    agrees::<_, Vec<u8>>(Uuid::from_u128(1));
    agrees::<_, Vec<u8>>(be_block_store::Hash::from_bytes([2; 32]));
    agrees::<_, Vec<u8>>(be_block_commit::CommitId::from_hash(
        be_block_store::Hash::from_bytes([3; 32]),
    ));

    for access in [
        Access::None,
        Access::KnowExists,
        Access::Join,
        Access::View,
        Access::Edit,
        Access::Manage,
    ] {
        agrees::<_, generated::shared::access::AccessLevel>(access);
    }
    for parent in [
        BlockParent::Root,
        BlockParent::Detached,
        BlockParent::Block(Uuid::from_u128(4)),
    ] {
        agrees::<_, generated::types::BlockParent>(parent);
    }
    agrees::<_, generated::types::EscrowKey>(EscrowKey {
        id: Uuid::from_u128(5),
        epoch: 6,
        public: [7; 32],
        head: [8; 32],
        maker: Uuid::from_u128(9),
        signature: vec![10; 64],
    });
}
