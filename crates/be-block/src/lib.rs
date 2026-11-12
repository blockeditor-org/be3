use std::{collections::HashSet, error::Error, fmt};

use be_block_commit::MergeResult;
pub use be_block_store::{LoadedPart, Part, PartRef, Stored};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub mod blob;
pub mod block_url;
pub mod device_class;
pub mod display_settings;
pub mod editor_view;
pub mod history;
pub mod input_remapper;
pub mod input_settings;
pub mod metadata;
pub mod model;
pub mod organization;
pub mod plain;
pub mod presence;
pub mod profile;
pub mod resolution;
pub mod search_index;
pub mod setting;
pub mod settings;
pub mod settling;
pub mod streamed;
pub mod version_control;
pub mod workspace_settings;

pub use be_block_graph::ANYONE_INSIDE;
pub use be_block_model;
pub use be_block_model::{Edit, Item, ObjectId, Touched};
pub use blob::{Blob, BlobKind, BlobOp};
pub use device_class::{ClassDefault, DeviceClass};
pub use display_settings::{DisplaySettings, DisplaySettingsContent, Motion, ThemeKind};
pub use editor_view::{EditorView, EditorViewContent, ViewState};
pub use input_remapper::{InputRemapper, InputRemapperContent};
pub use input_settings::{InputSettings, InputSettingsContent};
pub use metadata::{
    ARTIFACT, Artifact, BlockMetadata, CREATED_BY, DELETED_FROM, Definition, DeletedFrom,
    EXECUTABLE, Entries, ICON, ICON_COLOR, INTRINSIC_NAME, LOCAL_ID, MATERIAL_ICON, MY_FILES,
    MetaKey, NAME, Origin, ROW_PREFIX, SCHEMA, SEARCH_INDEX, SETTINGS, THUMBHASH, Thumbhash,
    VIEW_EDITOR,
};
pub use model::Root;
pub use organization::{
    Allowance, LostAccess, OfflineChoice, OfflinePolicy, OfflineSettings, OfflineSettingsContent,
    OrganizationPolicies, OrganizationPoliciesContent, SYSTEM_TYPES, UNFILED_TYPE, WhoMay,
    is_workspace_type, my_files, my_files_of, organization_settings, unfiled, unfiled_of,
};
pub use profile::{
    DESKTOP_EDITOR, FILES_EDITOR, RecentBlock, Recents, VIEW_TYPE, WORKSPACE_EDITOR, WORKSPACE_TYPE,
};
pub use setting::{Layers, Setting, SettingsLayer};
pub use settings::{
    Follows, KindRule, KindVisibility, Layered, Resolution, ResolutionLayers, Settings,
    SettingsContent, SettingsProfile, SettingsProfileContent, SettingsSource, holds_owner_only,
    layered,
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

/// How much of its stored form a replica of a type loads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Loads {
    Whole,
    /// Only the root's first part (`Part::Node`'s first child), such as a file's header; the
    /// rest stays in the store. `from_stored` gets a `Stored` holding only that part's bytes,
    /// `parts` hands the rest back as `Part::Reused`, and editors read it a range at a time
    /// (guides/blocks/apps.md, "Reading a range").
    FirstPart,
}

pub trait BlockContent: Sized + Send + Sync + 'static {
    const CONTENT_TYPE: Uuid;

    const LOADS: Loads = Loads::Whole;

    const FROZEN: bool = false;

    fn schema() -> String {
        String::new()
    }

    fn encode(&self) -> Vec<u8>;

    fn decode(bytes: &[u8]) -> Result<Self, ContentError>;

    /// The stored form as parts to save: their leaves, in order, are `encode()`, and a part
    /// unchanged since `from_stored` can be handed back as `Part::Reused`, so a save seals and
    /// uploads only what changed. Every replica must choose the same parts for the same
    /// content, since parts are sealed deterministically (decisions/deterministic_seal_nonce.md).
    fn parts(&self) -> Part<'_> {
        Part::from(self.encode())
    }

    /// Decodes stored content, keeping the refs of the parts it was saved in for `parts`.
    fn from_stored(stored: &Stored) -> Result<Self, ContentError> {
        Self::decode(&stored.bytes)
    }

    /// The bytes of a part this content hands back as `Part::Reused` and holds anyway, such as
    /// one it rebuilt from an operation that named the part by its hash and found to match
    /// (guides/type_modules.md, "Parts named by hash"), so reading it needs no `read-part`.
    fn reused_bytes(&self, part: PartRef) -> Option<std::borrow::Cow<'_, [u8]>> {
        let _ = part;
        None
    }

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

    /// The searchable text of the content, one `SearchDocument` per key, which the search
    /// index holds (plans/search.md); none for a type that is not content-searchable. Content
    /// that loads only its first part reads the others with `parts`. `keys` (none for every
    /// document) and `indexed` (the parts the index already holds text from) only spare work:
    /// a type need not answer documents outside `keys`, nor read a part in `indexed`, so a
    /// snippet reads one part of a large block and reindexing it reads only its changed
    /// parts. The host drops documents outside `keys` and the bodies of documents whose source
    /// is in `indexed` whatever the type answers.
    fn search_text(
        &self,
        keys: Option<&HashSet<String>>,
        indexed: &HashSet<PartRef>,
        parts: PartReader,
    ) -> impl Future<Output = Result<Vec<SearchDocument>, String>> + 'static {
        let _ = (keys, indexed, parts);
        std::future::ready(Ok(Vec::new()))
    }
}

/// A searchable document of a block's content (`BlockContent::search_text`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchDocument {
    /// Stable across edits and rekeys, unlike a part's ref: a chat segment's number, a long
    /// text's section id, or empty for a block that is one document.
    pub key: String,
    /// The part the document's text came from, if it is one part.
    pub source: Option<PartRef>,
    /// The document's own name, such as a section's title. A block's name is indexed from
    /// the block, so a block that is one document leaves this out.
    pub name: Option<String>,
    /// None when `source` is one of the parts already indexed, whose text the index keeps.
    pub body: Option<String>,
}

impl SearchDocument {
    /// The one document of a block that is one: its whole text.
    pub fn whole(body: String) -> Self {
        Self {
            body: Some(body),
            ..Self::default()
        }
    }
}

pub fn schema_fingerprint<C: BlockContent>() -> String {
    be_block_store::Hash::of(C::schema().as_bytes()).to_hex()[..16].to_owned()
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
    Reorder { block: Uuid, after: Option<Uuid> },
}

/// What an edit made on this device tells the block's other members who are not connected, once
/// the server holds it: the text of a notification and the accounts it mentions. `key` names
/// what is announced (chat: the message), unique in the block and the same each time the edit
/// is made again (a host replays an editor's edits into a restarted replica), so it is told
/// once.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Announcement {
    pub key: String,
    pub text: String,
    pub mentions: Vec<Uuid>,
}

pub trait LiveEdit: BlockContent {
    type Op: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    fn apply(&mut self, operation: &Self::Op);

    /// What `operation` announces (`Announcement`), asked when the edit is made on this device
    /// of content that holds it. The replica hands it to the host with the edit, and the host
    /// keeps it beside the journal entry or commit holding the edit and sends it once the
    /// server holds that, whether the replica is open then or not.
    fn announcements(&self, operation: &Self::Op) -> Vec<Announcement> {
        let _ = operation;
        Vec::new()
    }

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

    /// An operation made on `before`, as `client` applies it to `after`: the same visible
    /// content loaded afresh when a session restarts, so whatever names positions must move (a
    /// sequence edit travels as the visible splices it made). None when nothing of it is left.
    /// Content that keeps no session state carries operations over unchanged.
    fn carry_over(
        operation: &Self::Op,
        before: &Self,
        after: &Self,
        client: u64,
    ) -> Option<Self::Op> {
        let _ = (before, after, client);
        Some(operation.clone())
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

/// Reads a range of a saved part's bytes, for content that loaded only its first part
/// (`Loads::FirstPart`): the host's `read-part`.
#[derive(Clone)]
pub struct PartReader(std::rc::Rc<dyn Fn(PartRef, u64, u64) -> PartRead>);

pub type PartRead = std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, String>>>>;

impl PartReader {
    pub fn new(read: impl Fn(PartRef, u64, u64) -> PartRead + 'static) -> Self {
        Self(std::rc::Rc::new(read))
    }

    pub fn read(&self, part: PartRef, offset: u64, length: u64) -> PartRead {
        (self.0)(part, offset, length)
    }

    /// The whole part.
    pub fn read_all(&self, part: PartRef) -> PartRead {
        self.read(part, 0, part.length())
    }
}

/// `merge3` over parts, for content that loads only its first part: each side as
/// `from_stored` decoded it from that part, reading whatever else it needs with `parts`. The
/// merged value hands back the parts it kept as they were as `Part::Reused`, so writing it
/// uploads only what the merge changed (guides/type_modules.md, "Merging by parts").
pub trait MergeParts: Merge {
    fn merge_parts(
        base: Self,
        ours: Self,
        theirs: Self,
        parts: PartReader,
    ) -> impl Future<Output = Result<MergeResult<Self>, String>> + 'static;
}

#[cfg(test)]
mod tests;
