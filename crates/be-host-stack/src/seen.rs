//! What the page reads (`Seen`), and how the worker hands it over without the page ever
//! taking a lock the worker holds (plans/web_page_never_waits.md).
//!
//! The worker keeps `Seen` inside its `Shared` and hands it over in bytes, as it will to a page
//! whose memory it does not share: the whole `Seen` first, then each time what changed since the
//! last (`Update`, `diff`), onto a lock-free stack (`Handover`) the page takes every update from
//! at once. Neither side ever waits on the other. The large parts are `Part`s and persistent
//! maps, which the worker's last handed-over copy shares until the worker changes them, so
//! working out what changed walks only what the worker wrote.
//!
//! The worker publishes at the end of each pass of its loop, and before the page hears of
//! anything from the worker thread: a change notice (`changes::post`), a wake, or a reply to a
//! page call (`hold_answer`) is held until the update that has it is handed over. So a read
//! changes when the update lands, as a signal does, and the page never hears of a change it
//! cannot read yet.
//!
//! The page keeps the worker's `Seen` as its updates made it, and what it changed itself since
//! (`Edit`s), each sent to the worker as a command too: its own view is that `Seen` with the
//! edits the worker had not applied by then laid over it again.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
    sync::{
        Arc, PoisonError,
        atomic::{AtomicPtr, AtomicUsize, Ordering},
    },
};

use be_block_content::{BlockMetadata, Entries};
use be_block_graph::BlockParent;
use be_block_type_host::Conflict;
use be_sdk_type_api::{Description, EditorId};
use be_wasm_threads::Mutex;
use uuid::Uuid;

use crate::{
    Content, Graph, Node, Owner, Query, SearchAsk, Searched,
    conflicts::Comparison,
    revisions::Preview,
    version::VersionState,
    worker::{History, PairingRequest, Shared},
};

/// A part of `Seen` the worker and the copies it handed over share until the worker changes
/// it: a write copies it first unless nothing else holds it (`Arc::make_mut`).
pub struct Part<T>(Arc<T>);

impl<T> Part<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(value))
    }
}

impl<T: Default> Default for Part<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Clone for Part<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> Deref for Part<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> DerefMut for Part<T> {
    fn deref_mut(&mut self) -> &mut T {
        Arc::make_mut(&mut self.0)
    }
}

impl<T: serde::Serialize> serde::Serialize for Part<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Part<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::new)
    }
}

impl<T: PartialEq> PartialEq for Part<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

/// A part the worker did not write since the last `Seen` it handed over is the same `Arc` in
/// both, and sends nothing.
impl<T: crate::diff::Diff + Clone> crate::diff::Diff for Part<T> {
    type Delta = T::Delta;

    fn diff(old: &Self, new: &Self) -> Option<T::Delta> {
        if Arc::ptr_eq(&old.0, &new.0) {
            return None;
        }
        T::diff(&old.0, &new.0)
    }

    fn apply(&mut self, delta: T::Delta) {
        T::apply(Arc::make_mut(&mut self.0), delta);
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Part<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Words the stack has fixed, or made up as it runs: what `Seen` says in words, which is
/// handed over in bytes as well as by reference (`Seen::encode`).
pub type Words = std::borrow::Cow<'static, str>;

/// A block whose replica owes a seal, or holds edits the device's journal does not hold yet,
/// with what it waits on in words (`be_block_type_host::Owing`).
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Owed {
    pub block: Uuid,
    pub said: String,
}

impl From<&be_block_type_host::Owing> for Owed {
    fn from(owing: &be_block_type_host::Owing) -> Self {
        Self {
            block: owing.block,
            said: owing.to_string(),
        }
    }
}

impl std::fmt::Display for Owed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.said)
    }
}

/// An edit that could not be sent (`be_block_client::UnsentBlock`), as the page reads it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Unsent {
    pub block: Uuid,
    pub name: Option<String>,
    pub since: i64,
    pub offerable: bool,
    pub refused: Option<be_block_protocol::ErrorCode>,
}

impl From<&be_block_client::UnsentBlock> for Unsent {
    fn from(unsent: &be_block_client::UnsentBlock) -> Self {
        Self {
            block: unsent.block,
            name: unsent.name.clone(),
            since: unsent.since,
            offerable: unsent.offerable,
            refused: unsent.refused,
        }
    }
}

/// A moment on the monotonic clock, handed over as how long from now: two instances of the
/// module count from the same origin on the web, but an `Instant` is not bytes.
/// A moment as nanoseconds from now, negative for one already past: a held clock stands still
/// behind the real one, so what is due on it is often in the past. The page and its worker
/// read one clock.
pub(crate) mod moment {
    use std::time::{Duration, Instant};

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    fn from_now(at: Instant) -> i128 {
        let now = Instant::now();
        match at.checked_duration_since(now) {
            Some(ahead) => ahead.as_nanos() as i128,
            None => -(now.duration_since(at).as_nanos() as i128),
        }
    }

    fn at<E: serde::de::Error>(from_now: i128) -> Result<Instant, E> {
        let now = Instant::now();
        let span = Duration::from_nanos(from_now.unsigned_abs().min(u64::MAX as u128) as u64);
        match from_now < 0 {
            true => now.checked_sub(span),
            false => now.checked_add(span),
        }
        .ok_or_else(|| E::custom("a moment this clock cannot hold"))
    }

    pub(crate) fn serialize<S: Serializer>(at: &Instant, serializer: S) -> Result<S::Ok, S::Error> {
        from_now(*at).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Instant, D::Error> {
        at(i128::deserialize(deserializer)?)
    }

    pub(crate) mod option {
        use std::time::Instant;

        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        pub(crate) fn serialize<S: Serializer>(
            at: &Option<Instant>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            at.map(super::from_now).serialize(serializer)
        }

        pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<Instant>, D::Error> {
            Option::<i128>::deserialize(deserializer)?
                .map(super::at)
                .transpose()
        }
    }
}

/// Bytes behind an `Arc`, encoded as bytes rather than as a sequence of numbers.
pub(crate) mod bytes {
    use std::sync::Arc;

    use serde::{Deserialize, Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(
        bytes: &Arc<Vec<u8>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(bytes)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<Vec<u8>>, D::Error> {
        Ok(Arc::new(
            serde_bytes::ByteBuf::deserialize(deserializer)?.into_vec(),
        ))
    }
}

/// What an import staged for a block, shown until its replica has it (`Edit::ShowStaged`).
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Staged {
    pub content_type: Uuid,
    #[serde(with = "bytes")]
    pub bytes: Arc<Vec<u8>>,
}

/// An editor's frame the worker queued, numbered so the page can say which it took
/// (`Edit::TookFrames`).
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Frame {
    pub number: u64,
    pub block: Uuid,
    #[serde(with = "bytes")]
    pub bytes: Arc<Vec<u8>>,
}

/// An invitation code made or listed here, with the server and account whose own it is.
pub type OwnInvitation = (String, Uuid, be_block_keys::InviteCode);

crate::diff::whole!(
    Vec<Owed>,
    Vec<Unsent>,
    Vec<PairingRequest>,
    Vec<be_block_client::Contact>,
    Vec<OwnInvitation>,
    Vec<(Words, Uuid)>,
    Vec<Frame>,
    Vec<Conflict>,
    Option<be_block_protocol::AccountStorage>,
    Option<Comparison>,
    Option<HashMap<Uuid, be_block_protocol::NotifyMode>>,
    HashSet<Uuid>,
    HashMap<SearchAsk, Searched>,
    Preview,
    Content,
    Description,
    History,
    VersionState,
    Staged,
);

/// A moment sent whole when it changed, as an offset from now (`moment`).
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Due(#[serde(with = "moment::option")] Option<std::time::Instant>);

impl crate::diff::Diff for Option<std::time::Instant> {
    type Delta = Due;

    fn diff(old: &Self, new: &Self) -> Option<Due> {
        (old != new).then_some(Due(*new))
    }

    fn apply(&mut self, Due(due): Due) {
        *self = due;
    }
}

crate::diff::diffed! {
/// What the page's synchronous API reads: the part of the worker's `Shared` it hands over.
#[derive(Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Seen, delta SeenDelta {
    pub revision_previews: Part<imbl::OrdMap<Uuid, Preview>>,
    pub preview_aliases: Part<imbl::OrdMap<(Uuid, Uuid), Uuid>>,
    pub released_previews: Vec<Uuid>,
    pub graph: Part<Graph>,
    pub blocks: Part<imbl::OrdMap<Uuid, Content>>,
    pub described: Part<imbl::OrdMap<Uuid, Description>>,
    /// What the worker has for each editor that the page has not taken yet.
    pub editor_frames: Part<imbl::OrdMap<EditorId, Vec<Frame>>>,
    /// The number the next frame queued gets.
    pub frames_queued: u64,
    pub histories: Part<imbl::OrdMap<Uuid, History>>,
    pub wakes: u64,
    pub unsealed: usize,
    pub saves: u64,
    pub owing: usize,
    /// The blocks `owing` counts, with what their seals wait on, for `busy` to name.
    pub owing_blocks: Vec<Owed>,
    /// The blocks `undurable` counts, for `busy` to name.
    pub undurable_blocks: Vec<Owed>,
    /// How many `Command::SealOwed` the worker has carried out, for `busy` to say whether the
    /// host keeps asking.
    pub seals_owed: u64,
    /// What the worker waits on the type modules for (`Host::asking`), for `busy` to name.
    pub asking: Vec<String>,
    /// Open replicas with edits that are not in the device's journal yet.
    pub undurable: usize,
    pub connected: bool,
    /// Offline, the blocks this device holds no version of whole, so it can neither open nor
    /// change them (`BlockInfo::out_of_reach`); empty while connected.
    pub out_of_reach: imbl::OrdSet<Uuid>,
    pub reached: bool,
    /// A local account's stack, which has no server (`Config::is_local`): what it makes is saved
    /// as it is made, and nothing waits to reach a server.
    pub local: bool,
    /// Between connecting and having asked for everything the device must catch up on: set as
    /// `Session::arrive` starts, cleared once `arrive` has opened the wanted blocks and the
    /// outbox has drained and the open replicas have rejoined their sessions.
    pub arriving: bool,
    /// Set when a command ran or a seal moved edits into the outbox, until `report` has read
    /// the outbox into `waiting` again: `waiting` is stale meanwhile.
    pub recounting: bool,
    pub draining: bool,
    pub error: Option<String>,
    /// What went wrong sending: a refused or failed drain of the outbox, or a refused append to
    /// a session's journal. Unlike `error`, it is about being in step with the server, so it
    /// clears once the device is connected and nothing waits to be sent (`report`).
    pub sync_error: Option<String>,
    /// What the last drain that ran answered of what it did not send (`Drained` other than
    /// `Created`, `Published`, `Moved` and `Defined`), which `unsynced` names beside the outbox
    /// they leave waiting, with the errors recorded. A drain that fails as a whole leaves the
    /// outcomes of the one before, its error in `sync_error`.
    pub last_drain: Vec<String>,
    pub conflicts: Part<imbl::OrdMap<Uuid, Vec<Conflict>>>,
    pub pairing: Vec<PairingRequest>,
    pub contacts: Part<Vec<be_block_client::Contact>>,
    /// Invitation codes this stack's account made or listed (`crate::is_own_invitation`).
    pub own_invitations: Part<Vec<OwnInvitation>>,
    /// The account's storage, while an editor watches it (`watch_storage`).
    pub storage: Option<be_block_protocol::AccountStorage>,
    pub escrow_warning: Option<Words>,
    /// The account's notification mode for each block it set to other than `all`, once read
    /// (`Tasks::notify_modes`).
    pub notify_modes: Part<Option<HashMap<Uuid, be_block_protocol::NotifyMode>>>,
    /// Blocks an open replica found locked, until keys arrive (`Replicas::unlock`).
    pub locked: Part<HashSet<Uuid>>,
    pub versions: Part<imbl::OrdMap<Uuid, VersionState>>,
    /// How many commands the worker has finished: those the page sent and it has not finished
    /// are pending (`Shared::pending`).
    pub done: usize,
    pub typing: bool,
    /// While the host holds the modules' clock and nothing else is in flight, when their
    /// soonest timer that keeps a block in step comes due on it (`Modules::held_due`): what a
    /// stopped clock must reach for that work to happen. `None` while anything else could still
    /// make progress.
    #[serde(with = "moment::option")]
    pub modules_due: Option<std::time::Instant>,
    pub uploading: usize,
    pub listing_revisions: usize,
    /// Commands in the lane beside the loop that need only the server (`Served`).
    pub serving: usize,
    pub importing: usize,
    /// What the worker's loop awaits while it is not waiting for its next wake-up, for the
    /// busy reason that names what queued commands wait behind.
    pub step: Option<Words>,
    pub previewed: Part<HashSet<Uuid>>,
    pub staged: Part<imbl::OrdMap<Uuid, Staged>>,
    pub unreadable: Part<imbl::OrdMap<Uuid, String>>,
    pub waiting: usize,
    /// What the outbox held when `waiting` was counted, for `unsynced` to name.
    pub outbox: Vec<(Words, Uuid)>,
    /// Counts finished drains of the outbox.
    pub drains: u64,
    pub unsent: Part<Vec<Unsent>>,
    pub comparison: Option<Comparison>,
    /// The answers to the searches the page wants, as of `searched_at` (`Searching::shown`).
    pub searched: Part<HashMap<SearchAsk, Searched>>,
    /// Whether every edit this device queued for its search indexes is indexed.
    pub search_indexed: bool,
    /// Why the search is busy, as the worker's `busy` says.
    pub search_busy: Option<Words>,
    /// The last of the page's edits the worker applied (`Edit`).
    pub edits_applied: u64,
}
}

impl Seen {
    /// The `Seen` in bytes, for a page whose memory the worker does not share
    /// (plans/web_page_never_waits.md): postcard, the protocol crates' encoding.
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a Seen always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        postcard::from_bytes(bytes).map_err(|error| format!("a Seen did not decode: {error}"))
    }

    /// What keeps the device from being in step with the server, apart from `busy`: drive's
    /// `synced` waits until there is nothing.
    pub fn unsynced(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        // A local account has no server: in step with itself is in step.
        if !self.connected && !self.local {
            reasons.push(match &self.error {
                Some(error) => format!("the block store is not connected to the server: {error}"),
                None => "the block store is not connected to the server".to_owned(),
            });
        }
        if self.arriving {
            reasons.push("the block store catches up with the server after connecting".to_owned());
        }
        if self.draining {
            reasons.push("the block store sends the edits it kept".to_owned());
        }
        if self.serving > 0 {
            reasons.push(format!(
                "the block store waits on the server for {} command(s)",
                self.serving
            ));
        }
        if self.listing_revisions > 0 {
            reasons.push("the block store asks the server for a block's revisions".to_owned());
        }
        if self.recounting {
            reasons.push("the block store has yet to count what waits to be sent".to_owned());
        }
        let waiting = self.unsealed + self.waiting;
        if waiting > 0 {
            // Named in full, so a wait that never ends says what it waited on: whether the
            // edits were never sealed or sat in the outbox with no drain to send them.
            let outbox: Vec<String> = self
                .outbox
                .iter()
                .map(|(kind, block)| format!("{kind} {block}"))
                .collect();
            let unsealed: Vec<String> = self.owing_blocks.iter().map(ToString::to_string).collect();
            reasons.push(format!(
                "{waiting} edit(s) wait to be sent: {} unsealed [{}], {} in the outbox [{}]; {} drain(s) ended, {}; the last left [{}]; errors: {:?}, {:?}",
                self.unsealed,
                unsealed.join("; "),
                self.waiting,
                outbox.join(", "),
                self.drains,
                match self.draining {
                    true => "one under way",
                    false => "none under way",
                },
                self.last_drain.join(", "),
                self.error,
                self.sync_error,
            ));
        }
        reasons
    }

    /// Why the block store keeps a driver waiting, with `pending` commands not finished and the
    /// search busy for `searching`.
    pub(crate) fn busy_with(
        &self,
        hurried: bool,
        pending: usize,
        searching: Option<&str>,
    ) -> Vec<String> {
        let mut reasons: Vec<String> = [
            (pending > 0, "the block store has commands to carry out"),
            (self.typing, "a block type module has yet to settle"),
            (self.uploading > 0, "the block store uploads files"),
            (
                self.graph.fetching(),
                "the block store loads the organization's graph",
            ),
        ]
        .into_iter()
        .filter_map(|(busy, reason)| busy.then_some(reason.to_owned()))
        .collect();
        if self.undurable > 0 {
            let blocks: Vec<String> = self
                .undurable_blocks
                .iter()
                .map(ToString::to_string)
                .collect();
            reasons.push(format!(
                "the block store keeps edits the device's journal does not hold yet, for {}",
                blocks.join(", ")
            ));
        }
        if hurried && self.owing > 0 {
            let blocks: Vec<String> = self.owing_blocks.iter().map(ToString::to_string).collect();
            reasons.push(format!(
                "the block store owes edits a seal, for {} (sealed what was owed {} time(s))",
                blocks.join(", "),
                self.seals_owed
            ));
        }
        if let Some(searching) = searching {
            reasons.push(format!("the block store's search is busy: {searching}"));
        }
        if pending > 0
            && let Some(step) = &self.step
        {
            reasons.push(format!("the block store's loop is busy with {step}"));
        }
        if !self.asking.is_empty() {
            reasons.push(format!(
                "the block store waits on its type modules for {}",
                self.asking.join(", ")
            ));
        }
        reasons
    }

    /// What `block`'s content reads as: a revision's preview while it is one, else the open
    /// block's.
    pub(crate) fn content(&self, block: Uuid) -> Option<&Content> {
        match self.revision_previews.get(&block) {
            Some(preview)
                if !preview.released
                    && self
                        .graph
                        .get(preview.source)
                        .is_some_and(|node| node.access.can_view()) =>
            {
                preview.content.as_ref()
            }
            Some(_) => None,
            None => self.blocks.get(&block),
        }
    }
}

/// A change the page makes to what it reads at once, rather than waiting for the worker: the
/// page lays it over its own view, and sends it to the worker, which applies it to its `Seen`
/// as it takes the command (`Command::Edit`), in the order sent.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(crate) enum Edit {
    /// A block made here (`create`): placed under its parent unless the parent already lists it.
    Create(Node),
    /// A node as the page has it already (`duplicate`, `import`).
    Node(Node),
    Placing(Uuid, bool),
    Parent(Uuid, BlockParent),
    Define {
        block: Uuid,
        metadata: BlockMetadata,
        derived: Option<Entries>,
    },
    Derive(Uuid, Entries),
    Want(Owner, Vec<Query>),
    Stage {
        block: Uuid,
        content_type: Uuid,
        #[serde(with = "bytes")]
        bytes: Arc<Vec<u8>>,
    },
    /// Shows what an import staged for a block until its replica has it.
    ShowStaged(Uuid),
    Alias {
        root: Uuid,
        block: Uuid,
        id: Uuid,
    },
    /// A revision preview's child whose content is asked for (`revisions::load`).
    Requested(Uuid),
    /// Lets go of every preview under a preview's root (`revisions::release`).
    Release(Uuid),
    /// The released previews the host let go of (`revisions::collect_released`).
    Collected(Vec<Uuid>),
    /// Compares a block's conflicting sides, letting go of any comparison before.
    Compare(Uuid),
    StopComparing,
    /// One side of the comparison of `block` was previewed, or could not be.
    Compared {
        block: Uuid,
        theirs: bool,
        shown: Result<Uuid, String>,
    },
    /// Keeping a side of the comparison of `block` failed.
    KeepFailed {
        block: Uuid,
        error: String,
    },
    /// Forgets a block's conflicts (`conflicts::dismiss`).
    Dismiss(Uuid),
    /// The page took an editor's frames up to and including `upto`.
    TookFrames {
        editor: EditorId,
        upto: u64,
    },
}

impl Edit {
    /// Applies the edit, and answers the blocks whose content, or what is shown of it, changed.
    pub(crate) fn apply(&self, seen: &mut Seen) -> Vec<Uuid> {
        match self {
            Edit::Create(node) => {
                if let Some(BlockParent::Block(above)) = node.parent
                    && seen.graph.get(above).is_some_and(|above| {
                        !above.unfiled() && !above.references.contains(&node.id)
                    })
                {
                    seen.graph.placing(node.id, true);
                }
                seen.graph.change(node.clone());
                seen.graph.made(node.id);
            }
            Edit::Node(node) => seen.graph.change(node.clone()),
            Edit::Placing(block, placing) => seen.graph.placing(*block, *placing),
            Edit::Parent(block, parent) => {
                let placing = match parent {
                    BlockParent::Block(above) => seen
                        .graph
                        .get(*above)
                        .is_some_and(|above| !above.unfiled() && !above.references.contains(block)),
                    BlockParent::Root | BlockParent::Detached => false,
                };
                seen.graph.placing(*block, placing);
                seen.graph
                    .update(*block, |node| node.parent = Some(*parent));
            }
            Edit::Define {
                block,
                metadata,
                derived,
            } => {
                seen.graph
                    .update(*block, |node| node.metadata = metadata.clone());
                if let Some(derived) = derived {
                    seen.graph.derive(*block, derived.clone());
                }
            }
            Edit::Derive(block, derived) => seen.graph.derive(*block, derived.clone()),
            Edit::Want(owner, queries) => {
                if seen.graph.wanted(*owner) != queries.as_slice() {
                    seen.graph.want(*owner, queries.clone());
                }
            }
            Edit::Stage {
                block,
                content_type,
                bytes,
            } => {
                seen.staged.insert(
                    *block,
                    Staged {
                        content_type: *content_type,
                        bytes: Arc::clone(bytes),
                    },
                );
            }
            Edit::ShowStaged(block) => {
                if seen.blocks.contains_key(block) {
                    return Vec::new();
                }
                let Some(content) = seen
                    .staged
                    .get(block)
                    .filter(|staged| crate::is_known(staged.content_type))
                    .map(|staged| Content::new(staged.content_type, staged.bytes.to_vec()))
                else {
                    return Vec::new();
                };
                seen.blocks.insert(*block, content);
                seen.previewed.insert(*block);
                return vec![*block];
            }
            Edit::Alias { root, block, id } => {
                if seen.preview_aliases.contains_key(&(*root, *block)) {
                    return Vec::new();
                }
                seen.preview_aliases.insert((*root, *block), *id);
                let content = seen.blocks.get(block).cloned();
                seen.revision_previews.insert(
                    *id,
                    Preview {
                        source: *block,
                        root: *root,
                        content,
                        released: false,
                        requested: false,
                    },
                );
            }
            Edit::Requested(alias) => {
                if let Some(preview) = seen.revision_previews.get_mut(alias) {
                    preview.requested = true;
                }
            }
            Edit::Release(root) => release(seen, *root),
            Edit::Collected(ids) => {
                seen.released_previews.retain(|id| !ids.contains(id));
                for id in ids {
                    if let Some(preview) = seen.revision_previews.get_mut(id) {
                        preview.content = None;
                    }
                    seen.unreadable.remove(id);
                }
            }
            Edit::Compare(block) => {
                stop_comparing(seen);
                seen.comparison = Some(Comparison {
                    block: *block,
                    ours: None,
                    theirs: None,
                    error: None,
                });
                crate::changes::stale();
            }
            Edit::StopComparing => stop_comparing(seen),
            Edit::Compared {
                block,
                theirs,
                shown,
            } => match seen
                .comparison
                .as_mut()
                .filter(|comparison| comparison.block == *block)
            {
                Some(comparison) => {
                    let slot = match theirs {
                        false => &mut comparison.ours,
                        true => &mut comparison.theirs,
                    };
                    *slot = Some(shown.clone());
                    crate::changes::stale();
                }
                None => {
                    if let Ok(preview) = shown {
                        release(seen, *preview);
                    }
                }
            },
            Edit::KeepFailed { block, error } => {
                if let Some(comparison) = seen
                    .comparison
                    .as_mut()
                    .filter(|comparison| comparison.block == *block)
                {
                    comparison.error = Some(error.clone());
                    crate::changes::stale();
                }
            }
            Edit::Dismiss(block) => {
                stop_comparing(seen);
                if seen.conflicts.contains_key(block) {
                    seen.conflicts.remove(block);
                    crate::changes::stale();
                    return vec![*block];
                }
            }
            Edit::TookFrames { editor, upto } => {
                let Some(frames) = seen.editor_frames.get(editor) else {
                    return Vec::new();
                };
                if frames.first().is_none_or(|frame| frame.number > *upto) {
                    return Vec::new();
                }
                let frames = seen
                    .editor_frames
                    .get_mut(editor)
                    .expect("the editor has frames");
                frames.retain(|frame| frame.number > *upto);
                if frames.is_empty() {
                    seen.editor_frames.remove(editor);
                }
            }
        }
        Vec::new()
    }
}

fn release(seen: &mut Seen, root: Uuid) {
    let released: Vec<Uuid> = seen
        .revision_previews
        .iter()
        .filter(|(_, preview)| preview.root == root && !preview.released)
        .map(|(id, _)| *id)
        .collect();
    for id in released {
        if let Some(preview) = seen.revision_previews.get_mut(&id) {
            preview.released = true;
        }
        seen.released_previews.push(id);
    }
    crate::changes::wake();
}

fn stop_comparing(seen: &mut Seen) {
    let Some(comparison) = seen.comparison.take() else {
        return;
    };
    crate::changes::stale();
    for preview in [comparison.ours, comparison.theirs]
        .into_iter()
        .flatten()
        .flatten()
    {
        release(seen, preview);
    }
}

/// What the worker hands over: its whole `Seen` first, then what changed since the one before.
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) enum Update {
    Whole(Seen),
    Changed(SeenDelta),
}

/// An update as it is handed over: what changed in the `Seen`, with what the worker tells the
/// page of it, which the page hears once it has applied the change.
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct Handed {
    pub(crate) update: Option<Update>,
    /// What changed, for the page's watchers (`changes::post`).
    pub(crate) changes: crate::Changes,
    /// Answers to the page's commands by reply number, `None` for a reply dropped unanswered
    /// (`replies::Reply`).
    pub(crate) answers: Vec<(u64, Option<Vec<u8>>)>,
    /// In a debug build, now and then (every time in this crate's tests) the worker's whole
    /// `Seen` as well, which the page checks what it made of the updates against: a field the
    /// diff left out cannot drift apart unnoticed.
    #[cfg(debug_assertions)]
    pub(crate) check: Option<Seen>,
}

/// How often a debug build's update carries the whole `Seen` to check against (`Handed::check`).
#[cfg(all(debug_assertions, test))]
const CHECK_EVERY: u64 = 1;
#[cfg(all(debug_assertions, not(test)))]
const CHECK_EVERY: u64 = 64;

/// The updates the worker handed over and the page has not taken, in bytes, as they will travel
/// to a page whose memory the worker does not share: a lock-free stack the worker pushes each
/// onto and the page takes all of at once, so neither ever waits on the other. A page that runs
/// no frames (a hidden tab) takes nothing, so past `HELD_UPDATES` updates or `HELD_BYTES` bytes
/// the worker puts its whole `Seen` in their place in one swap (`publish`, `replace`): a whole
/// `Seen` is a valid next update whatever the page took.
#[derive(Default)]
pub(crate) struct Handover {
    head: AtomicPtr<Link>,
    /// How many updates, and how many bytes of them, were put since the last take.
    held: AtomicUsize,
    held_bytes: AtomicUsize,
    /// Natively, how many updates were put and whether the page closed the handover, for a
    /// thread that waits on an answer to wait on (`wait_past`). The web's page never waits.
    #[cfg(not(target_arch = "wasm32"))]
    put: (std::sync::Mutex<(u64, bool)>, std::sync::Condvar),
    /// How many updates were put, and who awaits the next (`handed_past`): what takes the
    /// updates while no frame does (`taking_updates`).
    handed: std::sync::atomic::AtomicU64,
    woken: futures_util::task::AtomicWaker,
    /// Frames take the updates (`take_updates_in_frame`), so a test's thread that takes them
    /// for a test that runs none stops.
    frames_take: std::sync::atomic::AtomicBool,
}

/// How many updates the handover holds before the worker gathers them into one (`Handover`).
const HELD_UPDATES: usize = 64;
/// How many bytes of updates the handover holds before the worker gathers them into one.
const HELD_BYTES: usize = 4 << 20;

struct Link {
    bytes: Vec<u8>,
    next: *mut Link,
}

impl Handover {
    pub(crate) fn put(&self, bytes: Vec<u8>) {
        self.held.fetch_add(1, Ordering::AcqRel);
        self.held_bytes.fetch_add(bytes.len(), Ordering::AcqRel);
        let link = Box::into_raw(Box::new(Link {
            bytes,
            next: std::ptr::null_mut(),
        }));
        let mut head = self.head.load(Ordering::Acquire);
        loop {
            // `link` came from `Box::into_raw` above and is not in the stack yet, so only this
            // thread reaches it.
            // unsafe(memory)
            unsafe { (*link).next = head };
            match self
                .head
                .compare_exchange_weak(head, link, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(now) => head = now,
            }
        }
        self.signal();
    }

    /// Tells whoever waits that an update was put.
    fn signal(&self) {
        self.handed.fetch_add(1, Ordering::AcqRel);
        self.woken.wake();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (count, put) = &self.put;
            count.lock().unwrap_or_else(PoisonError::into_inner).0 += 1;
            put.notify_all();
        }
    }

    pub(crate) fn frames_take_now(&self) {
        self.frames_take.store(true, Ordering::Release);
    }

    #[cfg_attr(
        not(all(any(test, feature = "testing"), not(target_arch = "wasm32"))),
        allow(dead_code)
    )]
    pub(crate) fn frames_take(&self) -> bool {
        self.frames_take.load(Ordering::Acquire)
    }

    /// How many updates were put so far, to await past (`handed_past`).
    pub(crate) fn handed(&self) -> u64 {
        self.handed.load(Ordering::Acquire)
    }

    /// Ready once more than `handed` updates were put.
    pub(crate) async fn handed_past(&self, handed: u64) {
        std::future::poll_fn(|context| {
            if self.handed() > handed {
                return std::task::Poll::Ready(());
            }
            self.woken.register(context.waker());
            match self.handed() > handed {
                true => std::task::Poll::Ready(()),
                false => std::task::Poll::Pending,
            }
        })
        .await;
    }

    /// How many updates were put so far, to wait past (`wait_past`).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn puts(&self) -> u64 {
        self.put.0.lock().unwrap_or_else(PoisonError::into_inner).0
    }

    /// Waits until more than `puts` updates were put, the handover is closed or `until` passes,
    /// and answers whether one was put.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn wait_past(&self, puts: u64, until: Option<std::time::Instant>) -> bool {
        let (count, put) = &self.put;
        let mut held = count.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if held.0 > puts {
                return true;
            }
            if held.1 {
                return false;
            }
            held = match until {
                None => put.wait(held).unwrap_or_else(PoisonError::into_inner),
                Some(until) => {
                    let left = until.saturating_duration_since(std::time::Instant::now());
                    if left.is_zero() {
                        return false;
                    }
                    put.wait_timeout(held, left)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
        }
    }

    /// Ends every wait on the handover: the stack stopped.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn close(&self) {
        let (count, put) = &self.put;
        count.lock().unwrap_or_else(PoisonError::into_inner).1 = true;
        put.notify_all();
    }

    /// Puts `bytes`, a whole `Seen`, in place of every update the page has not taken, in one
    /// swap: the page takes either those or this, never neither. Only one thread puts at a time
    /// (`publish` holds the lock on `Shared`), so no put lands in between.
    /// Answers the updates it put `bytes` in place of, oldest first.
    fn replace(&self, bytes: Vec<u8>) -> Vec<Vec<u8>> {
        self.held.store(1, Ordering::Release);
        self.held_bytes.store(bytes.len(), Ordering::Release);
        let link = Box::into_raw(Box::new(Link {
            bytes,
            next: std::ptr::null_mut(),
        }));
        let mut old = self.head.swap(link, Ordering::AcqRel);
        let mut replaced = Vec::new();
        while !old.is_null() {
            // As in `take`: each link came from `Box::into_raw` in `put`, and the swap took the
            // whole stack out, so nothing else reaches it.
            // unsafe(memory)
            let held = unsafe { Box::from_raw(old) };
            old = held.next;
            replaced.push(held.bytes);
        }
        replaced.reverse();
        self.signal();
        replaced
    }

    /// Whether the updates not taken are past what the handover holds (`HELD_UPDATES`).
    fn crowded(&self) -> bool {
        self.held.load(Ordering::Acquire) >= HELD_UPDATES
            || self.held_bytes.load(Ordering::Acquire) >= HELD_BYTES
    }

    /// How many updates the handover holds, for a test.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        let mut link = self.head.load(Ordering::Acquire);
        let mut held = 0;
        while !link.is_null() {
            held += 1;
            // Only the test's thread takes while it counts, so no link goes meanwhile.
            // unsafe(memory)
            link = unsafe { (*link).next };
        }
        held
    }

    /// The updates handed over since the last take, oldest first.
    pub(crate) fn take(&self) -> Vec<Vec<u8>> {
        let mut link = self.head.swap(std::ptr::null_mut(), Ordering::AcqRel);
        self.held.store(0, Ordering::Release);
        self.held_bytes.store(0, Ordering::Release);
        let mut taken = Vec::new();
        while !link.is_null() {
            // Each link came from `Box::into_raw` in `put`, and the swap took the whole stack out,
            // so nothing else reaches it.
            // unsafe(memory)
            let held = unsafe { Box::from_raw(link) };
            link = held.next;
            taken.push(held.bytes);
        }
        taken.reverse();
        taken
    }
}

impl Drop for Handover {
    fn drop(&mut self) {
        self.take();
    }
}

/// What the worker tells the page with an update (`Handed`).
#[derive(Default)]
pub(crate) struct Told {
    pub(crate) changes: crate::Changes,
    pub(crate) answers: Vec<(u64, Option<Vec<u8>>)>,
}

/// Hands over what changed in the worker's `Seen` since the last one it handed over (the whole
/// `Seen` the first time) with what it tells the page of it, and answers whether it put an
/// update: nothing goes when nothing changed and there is nothing to tell. Called with the lock
/// on `Shared` held, so updates land in the order the state they copy changed.
pub(crate) fn publish(shared: &mut Shared, handover: &Handover, told: Told) -> bool {
    shared.refresh_seen();
    // The page has not taken what the handover holds: the worker hands over the whole `Seen` in
    // its place.
    let crowded = handover.crowded();
    let update = match &shared.handed {
        None => Some(Update::Whole(shared.seen.clone())),
        Some(_) if crowded => Some(Update::Whole(shared.seen.clone())),
        Some(last) => crate::diff::Diff::diff(last, &shared.seen).map(Update::Changed),
    };
    if update.is_none() && told.changes.is_empty() && told.answers.is_empty() {
        return false;
    }
    #[cfg(debug_assertions)]
    let check = shared
        .updates
        .is_multiple_of(CHECK_EVERY)
        .then(|| shared.seen.clone());
    shared.updates += 1;
    let handed = Handed {
        update,
        changes: told.changes,
        answers: told.answers,
        #[cfg(debug_assertions)]
        check,
    };
    let bytes = postcard::to_stdvec(&handed).expect("an update encodes");
    shared.handed = Some(shared.seen.clone());
    if !crowded {
        handover.put(bytes);
        return true;
    }
    // What the replaced updates told the page still has to reach it: in an update of its own
    // after the whole `Seen`, which has what they changed.
    let mut told = Told::default();
    for replaced in handover.replace(bytes) {
        let replaced: Handed = postcard::from_bytes(&replaced).expect("an update decodes");
        told.changes.merge(replaced.changes);
        told.answers.extend(replaced.answers);
    }
    if !told.changes.is_empty() || !told.answers.is_empty() {
        let handed = Handed {
            update: None,
            changes: told.changes,
            answers: told.answers,
            #[cfg(debug_assertions)]
            check: None,
        };
        handover.put(postcard::to_stdvec(&handed).expect("an update encodes"));
    }
    true
}

/// The worker thread's way to its stack, and what it holds back from the page until the next
/// `Seen` is handed over.
struct Here {
    shared: Arc<Mutex<Shared>>,
    handover: Arc<Handover>,
    changes: crate::Changes,
    wake: bool,
    answers: Vec<(u64, Option<Vec<u8>>)>,
    /// A task to publish is spawned and has not run yet.
    scheduled: bool,
}

thread_local! {
    static HERE: RefCell<Option<Here>> = const { RefCell::new(None) };
}

/// Marks this thread as the worker of the stack whose `Shared` this is, until dropped.
pub(crate) struct Worker;

impl Worker {
    pub(crate) fn enter(shared: &Arc<Mutex<Shared>>, handover: &Arc<Handover>) -> Self {
        HERE.with(|here| {
            *here.borrow_mut() = Some(Here {
                shared: Arc::clone(shared),
                handover: Arc::clone(handover),
                changes: crate::Changes::default(),
                wake: false,
                answers: Vec::new(),
                scheduled: false,
            });
        });
        Self
    }
}

impl Drop for Worker {
    /// The worker's last `Seen`, and what it held back for it: a task to publish would not run
    /// once the worker ends.
    fn drop(&mut self) {
        publish_now();
        HERE.with(|here| here.borrow_mut().take());
    }
}

/// On the worker thread, holds `changes` back until the next `Seen` is handed over, which it
/// asks for (`schedule`); elsewhere, answers them to be posted at once.
pub(crate) fn hold_changes(changes: crate::Changes) -> Option<crate::Changes> {
    let held = HERE.with(|here| match here.borrow_mut().as_mut() {
        Some(here) => {
            here.changes.merge(changes);
            None
        }
        None => Some(changes),
    });
    if held.is_none() {
        schedule();
    }
    held
}

/// On the worker thread, holds a wake of the host back until the next `Seen` is handed over,
/// and answers true.
pub(crate) fn hold_wake() -> bool {
    let held = HERE.with(|here| match here.borrow_mut().as_mut() {
        Some(here) => {
            here.wake = true;
            true
        }
        None => false,
    });
    if held {
        schedule();
    }
    held
}

/// On the worker thread, holds an answer to the page's reply `id` back until the next update,
/// which carries it, so what the page reads as it hears the answer has what the worker did
/// before it; elsewhere, hands it back.
pub(crate) fn hold_answer(id: u64, answer: Option<Vec<u8>>) -> Option<Option<Vec<u8>>> {
    let held = HERE.with(|here| match here.borrow_mut().as_mut() {
        Some(here) => {
            here.answers.push((id, answer));
            None
        }
        None => Some(answer),
    });
    if held.is_none() {
        schedule();
    }
    held
}

/// Has a task on the worker publish once the worker yields, unless one is due already: the
/// changes made until then go in one `Seen`, and the lock on `Shared` is not held then.
fn schedule() {
    let spawn = HERE.with(|here| {
        here.borrow_mut()
            .as_mut()
            .is_some_and(|here| !std::mem::replace(&mut here.scheduled, true))
    });
    if spawn {
        crate::platform::spawn_local(async {
            HERE.with(|here| {
                if let Some(here) = here.borrow_mut().as_mut() {
                    here.scheduled = false;
                }
            });
            publish_now();
        });
    }
}

/// Hands the worker's update over now, with the changes and answers held back for it, and
/// wakes the host to take it in its next frame. Only the worker calls it, and never while it
/// holds the lock on `Shared`: it may wait a moment for another thread to let go of it, as a
/// worker may.
pub(crate) fn publish_now() {
    let Some((shared, handover, told, wake)) = HERE.with(|here| {
        here.borrow_mut().as_mut().map(|here| {
            (
                Arc::clone(&here.shared),
                Arc::clone(&here.handover),
                Told {
                    changes: std::mem::take(&mut here.changes),
                    answers: std::mem::take(&mut here.answers),
                },
                std::mem::take(&mut here.wake),
            )
        })
    }) else {
        return;
    };
    let put = publish(
        &mut shared.lock().unwrap_or_else(PoisonError::into_inner),
        &handover,
        told,
    );
    if put || wake {
        be_host_base::host::wake();
    }
}

/// Whether this thread is a stack's worker.
pub(crate) fn on_worker() -> bool {
    HERE.with(|here| here.borrow().is_some())
}

/// On the worker thread, runs `change` on its `Shared` and hands the result over at once, and
/// answers true: an `Edit` the worker makes itself, or the invitations it notes. Elsewhere
/// answers false without running it. The worker never calls it while it holds the lock on
/// `Shared`.
pub(crate) fn here_mut(change: impl FnOnce(&mut Shared)) -> bool {
    let Some((shared, handover)) = HERE.with(|here| {
        here.borrow()
            .as_ref()
            .map(|here| (Arc::clone(&here.shared), Arc::clone(&here.handover)))
    }) else {
        return false;
    };
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    change(&mut held);
    if publish(&mut held, &handover, Told::default()) {
        be_host_base::host::wake();
    }
    true
}

/// On the worker thread, reads its `Seen`.
pub(crate) fn here<T>(read: impl FnOnce(&Seen) -> T) -> Option<T> {
    let shared = HERE.with(|here| here.borrow().as_ref().map(|here| Arc::clone(&here.shared)))?;
    let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    Some(read(&held.seen))
}
