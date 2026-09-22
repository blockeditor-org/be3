use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const NAME: Uuid = Uuid::from_u128(0x6e61_6d65_5f5f_5f5f_5f5f_5f5f_5f5f_5f5f);

pub const MAX_NAME_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockName {
    pub manual: bool,
    pub value: String,
}

pub fn read_name(properties: &BTreeMap<Uuid, Vec<u8>>) -> Option<BlockName> {
    let bytes = properties.get(&NAME)?;
    serde_json::from_slice::<BlockName>(bytes)
        .ok()
        .filter(|name| !name.value.is_empty())
}

pub fn implicit_name_update(
    properties: &BTreeMap<Uuid, Vec<u8>>,
    implicit_name: Option<String>,
) -> Option<Vec<u8>> {
    let current = read_name(properties);
    if current.as_ref().is_some_and(|name| name.manual) {
        return None;
    }
    let mut value = implicit_name.unwrap_or_default();
    if value.len() > MAX_NAME_BYTES {
        let mut end = MAX_NAME_BYTES;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
    }
    let unchanged = current.map_or(value.is_empty(), |name| name.value == value);
    (!unchanged).then(|| {
        encode_name(&BlockName {
            manual: false,
            value,
        })
    })
}

pub fn encode_name(name: &BlockName) -> Vec<u8> {
    serde_json::to_vec(name)
        .unwrap_or_else(|error| crate::fatal(format!("failed to encode block name: {error}")))
}

pub(crate) fn apply_implicit_name(
    properties: &mut BTreeMap<Uuid, Vec<u8>>,
    implicit_name: Option<String>,
) {
    if read_name(properties).is_some_and(|name| name.manual) {
        return;
    }
    match implicit_name {
        Some(value) => {
            properties.insert(
                NAME,
                encode_name(&BlockName {
                    manual: false,
                    value,
                }),
            );
        }
        None => {
            properties.remove(&NAME);
        }
    }
}

#[cfg(test)]
mod tests;
