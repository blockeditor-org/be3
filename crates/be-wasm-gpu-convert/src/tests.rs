use super::*;

mod every_texture_and_vertex_format_comes_back_as_it_went;

/// Every case of a plain ABI enum, found by decoding each index in turn as postcard writes a
/// variant's: the first index that is no case ends them.
pub(super) fn every<T: serde::de::DeserializeOwned>() -> Vec<T> {
    (0u8..=127)
        .map_while(|index| be_wasm_ops::decode(&[index]))
        .collect()
}
