use sha2::{Digest as _, Sha256};

use crate::format::TextureKey;

pub fn fingerprint(size: [u32; 2], pixels: &[[u8; 4]]) -> TextureKey {
    let mut hash = Sha256::new();
    hash.update(size[0].to_le_bytes());
    hash.update(size[1].to_le_bytes());
    hash.update(pixels.as_flattened());
    u64::from_le_bytes(hash.finalize()[..8].try_into().unwrap())
}
