use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const LEN: usize = 32;

    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    pub fn of_parts(parts: &[&[u8]]) -> Self {
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update(part);
        }
        Self(hasher.finalize().into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        let mut hex = String::with_capacity(Self::LEN * 2);
        for byte in self.0 {
            hex.push(char::from_digit(u32::from(byte >> 4), 16).unwrap());
            hex.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap());
        }
        hex
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != Self::LEN * 2 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let high = hex[index * 2..].chars().next()?.to_digit(16)?;
            let low = hex[index * 2 + 1..].chars().next()?.to_digit(16)?;
            *byte = u8::try_from(high * 16 + low).ok()?;
        }
        Some(Self(bytes))
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Hash({})", self.to_hex())
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}
