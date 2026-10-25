use std::{error::Error, fmt};

use be_commit::MergeResult;
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub mod blob;
pub mod block_url;
pub mod device_class;
pub mod display_settings;
pub mod history;
pub mod input_remapper;
pub mod input_settings;
pub mod metadata;
pub mod model;
pub mod plain;
pub mod presence;
pub mod profile;
pub mod resolution;
pub mod setting;
pub mod settings;
pub mod settling;
pub mod streamed;
pub mod version_control;
pub mod workspace_settings;

pub use be_graph::ANYONE_INSIDE;
pub use be_model;
pub use be_model::{Edit, Item, ObjectId, Touched};
pub use blob::{Blob, BlobKind, BlobOp};
pub use device_class::{ClassDefault, DeviceClass};
pub use display_settings::{DisplaySettings, DisplaySettingsContent, Motion, ThemeKind};
pub use input_remapper::{InputRemapper, InputRemapperContent};
pub use input_settings::{InputSettings, InputSettingsContent};
pub use metadata::{
    ARTIFACT, Artifact, BlockMetadata, DELETED_FROM, Definition, DeletedFrom, EXECUTABLE, Entries,
    ICON, ICON_COLOR, INTRINSIC_NAME, LOCAL_ID, MATERIAL_ICON, MY_FILES, MetaKey, NAME, Origin,
    ROW_PREFIX, SCHEMA, SETTINGS, THUMBHASH, Thumbhash, VIEW_EDITOR,
};
pub use model::Root;
pub use profile::{
    FILES_EDITOR, LINUX_DESKTOP_EDITOR, RecentBlock, Recents, VIEW_TYPE, WORKSPACE_EDITOR,
    WORKSPACE_TYPE,
};
pub use setting::{Layers, Setting, SettingsLayer};
pub use settings::{
    Follows, KindRule, KindVisibility, Layered, Resolution, ResolutionLayers, Settings,
    SettingsContent, SettingsProfile, SettingsProfileContent, SettingsSource, layered,
};
pub use settling::{Landed, Settling};
pub use streamed::{
    HEADER_PREFIX_BYTES, Streamed, decode_streamed, encode_streamed, payload_start,
};
pub use version_control::{
    Checkout, CheckoutConflict, CheckoutContent, ConflictKind, MAIN_BRANCH, Repository,
    RepositoryContent,
};
pub use workspace_settings::{
    EDIT_KINDS, Home, HomeContent, PREFERRED_EDITOR, PREFIXES, PREFIXES_TYPE, PrefixRegistry,
    PrefixRegistryContent, TAG_COLOR, TAG_DEFINITIONS, TAGGED, TAGS_TYPE, Tag, TagColor,
    TagContent, Tags, TagsContent, edit_kind_key, normal_prefix,
};

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
