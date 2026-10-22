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
pub mod device_class;
pub mod display_settings;
pub mod editor_settings;
pub mod editor_view;
pub mod folder;
pub mod game;
pub mod history;
pub mod hotbar;
pub mod image;
pub mod input_remapper;
pub mod input_settings;
pub mod kinds;
pub mod logic_game;
pub mod logic_grid;
pub mod map;
pub mod metadata;
pub mod model;
pub mod pdf;
pub mod pixel_art;
pub mod pixel_ray_tracer;
pub mod presence;
pub mod presentation;
pub mod profile;
pub mod setting;
pub mod settings;
pub mod settling;
pub mod streamed;
pub mod text;
pub mod version_control;
pub mod video;
pub mod workspace;

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
pub use device_class::{ClassDefault, DeviceClass};
pub use display_settings::{DisplaySettings, DisplaySettingsContent, Motion, ThemeKind};
pub use editor_settings::{EditorSettings, EditorSettingsContent};
pub use editor_view::{EditorView, EditorViewContent, ViewState};
pub use folder::{Folder, FolderContent};
pub use game::{DeterministicGame, DeterministicGameContent, GameModuleContent, GameMove};
pub use hotbar::{Hotbar, HotbarContent, HotbarSlot, SlotKind};
pub use image::{ImageContent, ImageHeader, ImageOp};
pub use input_remapper::{InputRemapper, InputRemapperContent};
pub use input_settings::{InputSettings, InputSettingsContent};
pub use kinds::{BeuiDemoContent, PanZoomContent, Scene3dContent, TriangleContent};
pub use logic_game::{LogicGameContent, LogicGameProgress};
pub use logic_grid::{LogicGridContent, LogicGridDocument};
pub use map::{Map, MapContent};
pub use metadata::{
    ARTIFACT, ArtifactSource, BlockMetadata, DELETED_FROM, Definition, DeletedFrom, Entries, ICON,
    ICON_COLOR, INTRINSIC_NAME, LOCAL_ID, MY_FILES, MetaKey, NAME, Origin, ROW_PREFIX, SCHEMA,
    THUMBHASH, Thumbhash,
};
pub use model::Root;
pub use pdf::{PdfContent, PdfHeader};
pub use pixel_art::{PixelArtContent, PixelArtDocument};
pub use pixel_ray_tracer::PixelRayTracerContent;
pub use presentation::{Presentation, PresentationContent};
pub use profile::{FILES_EDITOR, LINUX_DESKTOP_EDITOR, RecentBlock, Recents, WORKSPACE_EDITOR};
pub use setting::{Layers, Setting, SettingsLayer};
pub use settings::{
    Layered, Settings, SettingsContent, SettingsProfile, SettingsProfileContent, layered,
};
pub use settling::{Landed, Settling};
pub use streamed::{
    HEADER_PREFIX_BYTES, Streamed, decode_streamed, encode_streamed, payload_start,
};
pub use text::{TextBlock, TextContent, TextIndentation, TextLanguage};
pub use version_control::{
    Checkout, CheckoutConflict, CheckoutContent, ConflictKind, MAIN_BRANCH, Repository,
    RepositoryContent,
};
pub use video::{VideoContent, VideoProject};
pub use workspace::{Workspace, WorkspaceContent};

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

    const FROZEN: bool = false;

    fn schema() -> String {
        String::new()
    }

    fn encode(&self) -> Vec<u8>;

    fn decode(bytes: &[u8]) -> Result<Self, ContentError>;

    fn references(&self) -> Vec<Uuid> {
        Vec::new()
    }

    fn references_in(&self, organization: Uuid) -> Vec<Uuid> {
        let _ = organization;
        self.references()
    }

    fn name(&self) -> Option<String> {
        None
    }

    fn describe(&self, metadata: &mut BlockMetadata) {
        let _ = metadata;
    }
}

pub fn schema_fingerprint<C: BlockContent>() -> String {
    be_store::Hash::of(C::schema().as_bytes()).to_hex()[..16].to_owned()
}

pub fn derived_metadata<C: BlockContent>(content: &C) -> Entries {
    let mut metadata = BlockMetadata::default();
    metadata.insert(&SCHEMA, &schema_fingerprint::<C>());
    if let Some(name) = content.name() {
        metadata.insert(&INTRINSIC_NAME, &name);
    }
    content.describe(&mut metadata);
    metadata.derived
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

    fn apply_beneath(
        &mut self,
        confirmed: &mut Self,
        operation: &Self::Op,
        local: &[Self::Op],
        touched: &mut Vec<Touched>,
    ) where
        Self: Clone,
    {
        confirmed.apply(operation);
        *self = confirmed.clone();
        for pending in local {
            self.apply(pending);
        }
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

    fn absorb_operation(operation: &mut Self::Op, next: Self::Op) -> Option<Self::Op> {
        let _ = operation;
        Some(next)
    }

    fn encode_operation(operation: &Self::Op) -> Vec<u8> {
        postcard::to_stdvec(operation).unwrap_or_default()
    }

    fn decode_operation(bytes: &[u8]) -> Result<Self::Op, ContentError> {
        postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("operation"))
    }

    fn session_state(&self) -> Vec<u8> {
        Vec::new()
    }

    fn adopt_session_state(&mut self, state: &[u8]) -> Result<(), ContentError> {
        let _ = state;
        Ok(())
    }
}

pub trait Undo: LiveEdit {
    type Step: Send + 'static;

    fn step(&self, operation: &Self::Op) -> Option<Self::Step>;

    fn absorb(previous: &mut Self::Step, next: Self::Step) -> Result<(), Self::Step> {
        let _ = previous;
        Err(next)
    }

    fn then(previous: &mut Self::Step, next: Self::Step);

    fn revert(&self, step: &Self::Step) -> Vec<Self::Op>;

    fn reapply(&self, step: &Self::Step) -> Vec<Self::Op>;
}

pub trait Merge: BlockContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self>;
}

#[cfg(test)]
mod tests;
