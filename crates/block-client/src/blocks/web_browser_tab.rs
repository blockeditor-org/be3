use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebBrowserTab {}

impl WebBrowserTab {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum WebBrowserTabOperation {}

impl Block for WebBrowserTab {
    type Operation = WebBrowserTabOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7765_622d_6272_6f77_7365_722d_7461_6201);

    fn apply_operation(_tab: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
