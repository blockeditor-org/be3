use std::{error::Error, fmt};

use be_commit::MergeResult;
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub mod audio;
pub mod blob;
pub mod block_url;
pub mod browser_tab;
pub mod calendar;
pub mod canvas;
pub mod checklist;
pub mod compiled_logic;
pub mod counter;
pub mod database;
pub mod database_schema;
pub mod database_view;
pub mod folder;
pub mod game;
pub mod hotbar;
pub mod image;
pub mod kinds;
pub mod logic_game;
pub mod logic_grid;
pub mod map;
pub mod metadata;
pub mod model;
pub mod paint;
pub mod pdf;
pub mod pixel_art;
pub mod pixel_ray_tracer;
pub mod presence;
pub mod presentation;
pub mod settings;
pub mod streamed;
pub mod text;
pub mod ui_settings;
pub mod video;

pub use audio::{AudioContent, AudioHeader};
pub use be_model;
pub use be_model::{Edit, Item, ObjectId, Touched};
pub use blob::{Blob, BlobKind, BlobOp};
pub use browser_tab::{BrowserTab, BrowserTabContent, HistoryItem};
pub use calendar::{Calendar, CalendarContent, CalendarEvent};
pub use canvas::CanvasContent;
pub use checklist::{Checklist, ChecklistContent, ChecklistItem};
pub use compiled_logic::{CompiledLogicContent, CompiledLogicDocument};
pub use counter::{Counter, CounterContent};
pub use database::{Database, DatabaseContent};
pub use database_schema::{DatabaseSchema, DatabaseSchemaContent};
pub use database_view::{DatabaseView, DatabaseViewContent};
pub use folder::{Folder, FolderContent};
pub use game::{DeterministicGame, DeterministicGameContent, GameModuleContent, GameMove};
pub use hotbar::{Hotbar, HotbarContent, HotbarSlot, SlotKind};
pub use image::{ImageContent, ImageHeader, ImageOp};
pub use kinds::{FileTreeContent, PanZoomContent, Scene3dContent, WorkspaceUiContent};
pub use logic_game::{LogicGameContent, LogicGameProgress};
pub use logic_grid::{LogicGridContent, LogicGridDocument};
pub use map::{Map, MapContent};
pub use metadata::{ArtifactSource, BlockMetadata, DerivedMetadata, Thumbhash};
pub use model::Root;
pub use paint::{PaintReview, PaintReviewContent, PaintSnapshotContent, PaintSnapshotHeader};
pub use pdf::{PdfContent, PdfHeader};
pub use pixel_art::{PixelArtContent, PixelArtDocument};
pub use pixel_ray_tracer::PixelRayTracerContent;
pub use presentation::{Presentation, PresentationContent};
pub use settings::{Settings, SettingsContent};
pub use streamed::{
    HEADER_PREFIX_BYTES, Streamed, decode_streamed, encode_streamed, payload_start,
};
pub use text::{TextContent, TextHeader, TextIndentation, TextLanguage, TextOp};
pub use ui_settings::{UiSettings, UiSettingsContent, Zoom};
pub use video::{VideoContent, VideoProject};

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

    fn references_in(&self, workspace: Uuid) -> Vec<Uuid> {
        let _ = workspace;
        self.references()
    }

    fn name(&self) -> Option<String> {
        None
    }

    fn derived_metadata(&self) -> DerivedMetadata {
        DerivedMetadata::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildChange {
    Add(Uuid),
    Delete(Uuid),
    Replace { old: Uuid, new: Uuid },
}

pub trait LiveEdit: BlockContent {
    type Op: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    fn apply(&mut self, operation: &Self::Op);

    fn apply_touching(&mut self, operation: &Self::Op, touched: &mut Vec<Touched>) {
        self.apply(operation);
        touched.push(Touched::Everything);
    }

    fn child_operations(&self, change: ChildChange) -> Option<Vec<Self::Op>> {
        let _ = change;
        None
    }

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
