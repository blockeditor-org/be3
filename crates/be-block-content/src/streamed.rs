use serde::{Serialize, de::DeserializeOwned};

use crate::{BlockContent, ContentError};

pub const HEADER_PREFIX_BYTES: usize = 4;

pub trait Streamed: BlockContent {
    type Header: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    fn header(&self) -> Self::Header;

    fn payload(&self) -> &[u8];

    fn from_parts(header: Self::Header, payload: Vec<u8>) -> Self;
}

pub fn encode_streamed<H: Serialize>(header: &H, payload: &[u8]) -> Vec<u8> {
    let encoded = postcard::to_stdvec(header).unwrap_or_default();
    let length = u32::try_from(encoded.len()).unwrap_or(u32::MAX);
    let mut bytes = Vec::with_capacity(HEADER_PREFIX_BYTES + encoded.len() + payload.len());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&encoded);
    bytes.extend_from_slice(payload);
    bytes
}

pub fn decode_streamed<H: DeserializeOwned>(bytes: &[u8]) -> Result<(H, Vec<u8>), ContentError> {
    let start = payload_start(bytes)?;
    if bytes.len() < start {
        return Err(ContentError::Truncated);
    }
    let header = postcard::from_bytes(&bytes[HEADER_PREFIX_BYTES..start])
        .map_err(|_| ContentError::Malformed("header"))?;
    Ok((header, bytes[start..].to_vec()))
}

pub fn payload_start(prefix: &[u8]) -> Result<usize, ContentError> {
    let length = prefix
        .get(..HEADER_PREFIX_BYTES)
        .ok_or(ContentError::Truncated)?;
    let length = u32::from_le_bytes(length.try_into().expect("four bytes")) as usize;
    HEADER_PREFIX_BYTES
        .checked_add(length)
        .ok_or(ContentError::Malformed("header length"))
}
