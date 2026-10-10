use bincode::Options;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::MAX_BLOB_BYTES;

pub trait HostValue: 'static {
    const KEY: &'static str;
    type Value: Clone + Default + PartialEq + Serialize + DeserializeOwned + 'static;
}

pub trait HostAction: Clone + Serialize + DeserializeOwned + 'static {
    const KEY: &'static str;
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostImage {
    pub width: u32,
    pub height: u32,
    #[serde(with = "serde_bytes")]
    pub rgba: Vec<u8>,
}

impl HostImage {
    pub fn is_whole(&self) -> bool {
        self.rgba.len() == self.width as usize * self.height as usize * 4
    }
}

pub fn encode_host<T: Serialize>(value: &T) -> Vec<u8> {
    codec().serialize(value).unwrap_or_default()
}

pub fn decode_host<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    codec().deserialize(bytes).ok()
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_BLOB_BYTES as u64)
        .reject_trailing_bytes()
}
