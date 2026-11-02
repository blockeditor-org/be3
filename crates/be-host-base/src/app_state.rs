use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod native;

pub use native::AppStateStore;

#[derive(Clone, Debug)]
pub struct AppStateError(String);

impl fmt::Display for AppStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AppStateError {}

impl From<String> for AppStateError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<rusqlite::Error> for AppStateError {
    fn from(error: rusqlite::Error) -> Self {
        Self(error.to_string())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum ServerLocation {
    Local,
    Remote(String),
}

impl ServerLocation {
    pub fn key(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Remote(url) => url,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SavedAccount {
    pub server: ServerLocation,
    pub id: Uuid,
    pub email: String,
    pub name: String,

    pub token: String,
    pub last_organization_id: Option<Uuid>,
    #[serde(default)]
    pub authorization: Option<String>,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
