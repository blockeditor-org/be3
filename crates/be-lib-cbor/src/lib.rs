//! CBOR as stored and sent: one value, nothing after it.
//!
//! `ciborium::from_reader` stops at the end of the first value and ignores whatever follows, so
//! two different byte strings would decode to the same value. Everything that reads a stored or
//! sent CBOR value reads it through [`decode`], which fails on that instead.

use serde::de::DeserializeOwned;

/// Why bytes are not one CBOR value of the type asked for.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// They do not begin with a value of the type.
    Value(String),
    /// A value of the type is followed by this many bytes.
    Trailing(usize),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Value(error) => f.write_str(error),
            Self::Trailing(count) => write!(f, "{count} bytes follow the value"),
        }
    }
}

impl std::error::Error for Error {}

/// The one value `bytes` holds, or an error when they hold no such value or anything follows it.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    let mut rest = bytes;
    let value =
        ciborium::from_reader(&mut rest).map_err(|error| Error::Value(error.to_string()))?;
    match rest.len() {
        0 => Ok(value),
        count => Err(Error::Trailing(count)),
    }
}

#[cfg(test)]
mod tests;
