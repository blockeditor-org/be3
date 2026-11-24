//! SHA-256 as the tools write it, in lowercase hex. Every digest a tool writes goes through
//! [`finish`], so they all spell it the same way.

use std::io::{self, Read};

pub use sha2::{Digest, Sha256};

/// The lowercase hex of the SHA-256 of what `hasher` has taken in.
pub fn finish(hasher: Sha256) -> String {
    hex::encode(hasher.finalize())
}

/// The lowercase hex of the SHA-256 of `bytes`.
pub fn sha256(bytes: &[u8]) -> String {
    finish(Sha256::new_with_prefix(bytes))
}

/// The lowercase hex of the SHA-256 of everything `reader` holds, and how many bytes that was.
pub fn sha256_read(mut reader: impl Read) -> io::Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let size = io::copy(&mut reader, &mut hasher)?;
    Ok((finish(hasher), size))
}

#[cfg(test)]
mod tests;
