//! A password as the person typed it: its bytes, zeroed when it is dropped and never printed
//! (`Debug` shows `Password(..)`). A root guest hands one to its host to check (`be3:root`'s
//! `host.unlock`, whose argument it is), and be-host checks it against the platform (the login
//! session's lock, PAM).
//!
//! Zeroing covers this value only: the text field it was typed into, the buffers that encoded
//! it and the frames that carried it are not scrubbed by it.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A password as typed. It is neither cloned nor compared: the host checks it against the
/// platform and drops it.
pub struct Password(Vec<u8>);

impl Password {
    /// The password `bytes`, as typed.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The bytes as typed.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The text as typed, if it is UTF-8, as a platform that checks text (PAM) takes it.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    /// Whether nothing was typed.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Password {
    fn from(typed: String) -> Self {
        Self(typed.into_bytes())
    }
}

// unsafe(memory)
impl Drop for Password {
    fn drop(&mut self) {
        for byte in self.0.iter_mut() {
            // Volatile, so the zeroing is not optimised away as a dead store.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Password(..)")
    }
}

/// As bytes, as `list<u8>` crosses be-wasm-ops' wire.
impl Serialize for Password {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Password {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde_bytes::ByteBuf::deserialize(deserializer).map(|bytes| Self(bytes.into_vec()))
    }
}

#[cfg(test)]
mod tests;
