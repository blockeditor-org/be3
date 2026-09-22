use std::{error::Error, fmt};

use be_commit::MergeResult;
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub mod browser_tab;
pub mod calendar;
pub mod checklist;
pub mod counter;
pub mod image;
mod keyed;
pub mod streamed;
pub mod text;
pub mod ui_settings;

pub use browser_tab::{BrowserTabContent, BrowserTabOp, HistoryItem};
pub use calendar::{CalendarContent, CalendarEvent, CalendarOp, CalendarStep};
pub use checklist::{ChecklistContent, ChecklistItem, ChecklistOp};
pub use counter::{CounterContent, CounterOp};
pub use image::{ImageContent, ImageHeader};
pub use streamed::{
    HEADER_PREFIX_BYTES, Streamed, decode_streamed, encode_streamed, payload_start,
};
pub use text::{TextContent, TextHeader, TextLanguage, TextOp};
pub use ui_settings::{UiSettingsContent, UiSettingsOp};

#[derive(Debug, Eq, PartialEq)]
pub enum ContentError {
    Malformed(&'static str),
    Truncated,
}

impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(what) => write!(formatter, "stored content is malformed: {what}"),
            Self::Truncated => formatter.write_str("stored content ended early"),
        }
    }
}

impl Error for ContentError {}

pub trait BlockContent: Sized + Send + Sync + 'static {
    const CONTENT_TYPE: Uuid;

    fn encode(&self) -> Vec<u8>;

    fn decode(bytes: &[u8]) -> Result<Self, ContentError>;

    fn references(&self) -> Vec<Uuid> {
        Vec::new()
    }

    fn name(&self) -> Option<String> {
        None
    }
}

pub trait LiveEdit: BlockContent {
    type Op: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    fn apply(&mut self, operation: &Self::Op);

    fn rebase(operation: Self::Op, onto: &[Self::Op]) -> Option<Self::Op> {
        let _ = onto;
        Some(operation)
    }

    fn encode_operation(operation: &Self::Op) -> Vec<u8> {
        postcard::to_stdvec(operation).unwrap_or_default()
    }

    fn decode_operation(bytes: &[u8]) -> Result<Self::Op, ContentError> {
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("operation"))
    }
}

pub trait Undo: LiveEdit {
    type Step: Send + 'static;

    fn step(&self, operation: &Self::Op) -> Option<Self::Step>;

    fn absorb(previous: &mut Self::Step, next: Self::Step) -> Result<(), Self::Step> {
        let _ = previous;
        Err(next)
    }

    fn revert(&self, step: &Self::Step) -> Vec<Self::Op>;

    fn reapply(&self, step: &Self::Step) -> Vec<Self::Op>;
}

pub trait Merge: BlockContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self>;
}

#[cfg(test)]
mod tests;
