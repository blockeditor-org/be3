use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "client",
    deps: ["../be-wasm-ops/wit"],
    network: true,
    with: {
        "uuid": uuid::Uuid,
        "hash": be_block_store::Hash,
        "commit-id": be_block_commit::CommitId,
        "public-key": [u8; 32],
        "entry-hash": [u8; 32],
        "access-level": be_block_graph::Access,
        "block-parent": be_block_graph::BlockParent,
        "escrow-key": crate::EscrowKey,
    },
    copy: ["uuid", "hash", "commit-id", "public-key", "entry-hash", "access-level", "block-parent"],
    default: [
        "key-set",
        "key-work",
        "session-state",
        "access-list",
        "organization-kind",
        "object-change",
        "reference-change",
    ],
    inline: [
        "relayed",
        "sealed",
        "device-poll",
        "blocks-query",
        "list-query",
        "tree-query",
        "keys-query",
        "revocations-query",
        "session-query",
        "create-block",
        "publish",
        "move",
        "grant-record",
        "role-record",
        "remove-member",
        "rotation",
        "new-escrow",
        "hold-objects",
        "create-organization",
        "create-agent",
        "create-link",
        "accept-link",
        "put-contact",
        "remove-contact",
        "set-recovery-key",
        "set-notify-mode",
        "session-target",
        "push-target",
        "tree-advanced",
        "session-happened",
        "followed",
    ],
});

pub use account::*;
pub use blocks::*;
pub use contacts::*;
pub use keys::*;
pub use members::*;
pub use notifications::*;
pub use objects::*;
pub use organizations::*;
pub use server::*;
pub use sessions::*;
pub use types::*;

pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// The most blocks one `query.blocks` names.
pub const MAX_SUMMARIES_ASKED: usize = 4096;
/// What a frame spends on more than its answer's items: the op's envelope and the lengths and
/// fixed fields around them. An answer sized to fit leaves this much of a frame spare.
pub const FRAME_ENVELOPE_BYTES: usize = 64 * 1024;
/// The blocks one block references, which its server refuses past (`create-block`,
/// `publish`) and a client before it sends them. Every summary lists them, so
/// with a block's largest metadata (64 KiB on the server) a summary still fits a frame on its
/// own, which every answer of many summaries sends alone if it must.
pub const MAX_REFERENCES: usize = 250_000;
/// The bytes `value` takes in a frame, whose values are postcard: what a server measures an
/// answer by so that it fits.
pub fn encoded_len<T: Serialize + ?Sized>(value: &T) -> usize {
    postcard::serialize_with_flavor(value, postcard::ser_flavors::Size::default())
        .unwrap_or(usize::MAX)
}
/// The largest object the server stores.
pub const MAX_OBJECT_BYTES: usize = 8 * 1024 * 1024;
/// The most objects one `reserve-uploads` or `download-urls` call names, which keeps its answer
/// well inside a frame.
pub const MAX_OBJECT_URLS: usize = 1024;
/// The largest sealed notification `notify` takes: what an app writes is about 1 KiB, and
/// the push carrying it must stay within the 4 KiB Web Push allows (RFC 8030), encryption
/// included.
pub const MAX_NOTIFICATION_BYTES: usize = 2048;
/// Keys one notification may name (`notify`), and how many the server remembers told per
/// account and block: as many as a device remembers sending (be-block-client
/// `refs/announcements.rs`, `REMEMBERED`). A device telling more at once names the newest.
pub const MAX_TOLD_KEYS: usize = 256;

/// What `notify` names of an announcement's `key` about `block`: a digest, so the server
/// keeps opaque bytes of a fixed size rather than anything a type wrote.
pub fn told_key(block: Uuid, key: &str) -> be_block_store::Hash {
    be_block_store::Hash::of_parts(&[block.as_bytes(), key.as_bytes()])
}

/// What one push carries to a device, encrypted to it (RFC 8291): the block's organization and
/// id, the account that sent it, which the server vouches for, then the notification sealed
/// under the block's data key. Bytes, so a service worker reads it without a decoder: 16 bytes
/// each of organization, block and sender, then the sealed notification. A device names the
/// sender and the block itself, from its own contacts and graph: the sealed part holds only
/// the text, bound to the organization, block, sender and time it was sent
/// (`be_block_client::Notification`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PushMessage {
    pub organization: Uuid,
    pub block: Uuid,
    pub sender: Uuid,
    pub sealed: Vec<u8>,
}

impl PushMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(48 + self.sealed.len());
        bytes.extend_from_slice(self.organization.as_bytes());
        bytes.extend_from_slice(self.block.as_bytes());
        bytes.extend_from_slice(self.sender.as_bytes());
        bytes.extend_from_slice(&self.sealed);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            organization: Uuid::from_slice(bytes.get(..16)?).ok()?,
            block: Uuid::from_slice(bytes.get(16..32)?).ok()?,
            sender: Uuid::from_slice(bytes.get(32..48)?).ok()?,
            sealed: bytes.get(48..)?.to_vec(),
        })
    }
}

impl ObjectChange {
    /// What turns the objects `before` holds into those `after` holds.
    pub fn between(before: &[be_block_store::Hash], after: &[be_block_store::Hash]) -> Self {
        let had: std::collections::HashSet<_> = before.iter().collect();
        let has: std::collections::HashSet<_> = after.iter().collect();
        let mut seen = std::collections::HashSet::new();
        Self {
            added: after
                .iter()
                .filter(|hash| !had.contains(hash) && seen.insert(**hash))
                .copied()
                .collect(),
            removed: before
                .iter()
                .filter(|hash| !has.contains(hash) && seen.insert(**hash))
                .copied()
                .collect(),
        }
    }

    /// `before` without what was removed, then what was added, each object once. Removing an
    /// object `before` lacks or adding one it has changes nothing.
    pub fn apply(&self, before: &[be_block_store::Hash]) -> Vec<be_block_store::Hash> {
        let removed: std::collections::HashSet<_> = self.removed.iter().collect();
        let mut seen = std::collections::HashSet::new();
        before
            .iter()
            .filter(|hash| !removed.contains(hash))
            .chain(&self.added)
            .filter(|hash| seen.insert(**hash))
            .copied()
            .collect()
    }
}

impl ReferenceChange {
    /// What turns the references `before` into `after`: those that keep their order (the
    /// longest run of `before` that `after` keeps in order) stay, every other one in `before`
    /// is removed and every other one in `after` is added at its index.
    pub fn between(before: &[Uuid], after: &[Uuid]) -> Self {
        let position: std::collections::HashMap<Uuid, usize> = before
            .iter()
            .enumerate()
            .rev()
            .map(|(index, reference)| (*reference, index))
            .collect();
        let shared: Vec<(usize, usize)> = after
            .iter()
            .enumerate()
            .filter_map(|(index, reference)| Some((index, *position.get(reference)?)))
            .collect();
        let kept: std::collections::HashSet<Uuid> = longest_increasing(&shared)
            .into_iter()
            .map(|index| after[index])
            .collect();
        let mut seen = std::collections::HashSet::new();
        Self {
            removed: before
                .iter()
                .filter(|reference| !kept.contains(reference) && seen.insert(**reference))
                .copied()
                .collect(),
            added: after
                .iter()
                .enumerate()
                .filter(|(_, reference)| !kept.contains(reference))
                .map(|(index, block)| PlacedReference {
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                    block: *block,
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.added.is_empty()
    }

    /// `before` without what was removed, with each added reference put in at its index (or at
    /// the end, past it), in one pass. An added reference `before` already holds moves to its
    /// index, since a client diffs against its own view of the list, which may already hold it;
    /// removing one `before` lacks changes nothing. A change `between` never makes, naming an
    /// added reference twice or with indices that do not increase, is none, and a server
    /// refuses it.
    pub fn apply(&self, before: &[Uuid]) -> Option<Vec<Uuid>> {
        let mut added = std::collections::HashSet::with_capacity(self.added.len());
        let canonical = self.added.iter().all(|placed| added.insert(placed.block))
            && self
                .added
                .windows(2)
                .all(|pair| pair[0].index < pair[1].index);
        if !canonical {
            return None;
        }
        let gone: std::collections::HashSet<&Uuid> = self.removed.iter().chain(&added).collect();
        let mut kept = before
            .iter()
            .filter(|reference| !gone.contains(reference))
            .copied();
        let mut merged = Vec::with_capacity(before.len() + self.added.len());
        for placed in &self.added {
            let index = usize::try_from(placed.index).unwrap_or(usize::MAX);
            while merged.len() < index
                && let Some(reference) = kept.next()
            {
                merged.push(reference);
            }
            merged.push(placed.block);
        }
        merged.extend(kept);
        Some(merged)
    }
}

/// The first items of the longest run of `pairs` whose second items increase, where each
/// pair is (an index in a new list, its position in an old one) and the pairs come in new
/// order.
fn longest_increasing(pairs: &[(usize, usize)]) -> Vec<usize> {
    let mut tails: Vec<usize> = Vec::new();
    let mut before = vec![None; pairs.len()];
    for (at, (_, old)) in pairs.iter().enumerate() {
        let length = tails.partition_point(|tail| pairs[*tail].1 < *old);
        if length > 0 {
            before[at] = Some(tails[length - 1]);
        }
        match tails.get_mut(length) {
            Some(tail) => *tail = at,
            None => tails.push(at),
        }
    }
    let mut run = Vec::with_capacity(tails.len());
    let mut at = tails.last().copied();
    while let Some(index) = at {
        run.push(pairs[index].0);
        at = before[index];
    }
    run.reverse();
    run
}

impl OrganizationRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Administrator => "Administrator",
            Self::Editor => "Editor",
        }
    }
}

/// Whether `key` names the scope key of an owner-only block. Its maker gives it a version-8 id
/// (`owner_only_key_id`), and the id is bound into every seal of the key, so a server cannot
/// pass the key off as an ordinary one to be escrowed, nor an ordinary key off as this.
pub fn owner_only_key(key: Uuid) -> bool {
    key.get_version_num() == 8
}

/// A fresh id for an owner-only block's scope key (`owner_only_key`).
pub fn owner_only_key_id() -> Uuid {
    uuid::Builder::from_custom_bytes(*Uuid::new_v4().as_bytes()).into_uuid()
}

impl BlockSummary {
    pub fn no_access(id: Uuid, content_type: Uuid, access: be_block_graph::Access) -> Self {
        Self {
            id,
            content_type,
            access,
            author: None,
            parent: None,
            head: None,
            references: Vec::new(),
            metadata: Vec::new(),
            version: None,
            key: None,
            scope: None,
            changed: 0,
            entry: None,
            held: true,
            opens_inside: false,
            sets_access: false,
            size: 0,
            tree_size: 0,
            owner_only: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EscrowKey {
    pub id: Uuid,
    pub epoch: u64,
    pub public: [u8; 32],
    pub signing: [u8; 32],
    /// The head of the organization's access log it was made under (guides/blocks/keys.md,
    /// "Epochs name their heads"): a device takes it, and seals it, only once it holds the log that
    /// far.
    pub head: [u8; 32],
    /// The administrator who made it, whose identity key signs `chained`.
    pub maker: Uuid,
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

impl EscrowKey {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("an escrow key always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }

    /// What its maker signs: the key and the log head it was made under, chained to the epoch
    /// before (`previous`, nil for the first).
    pub fn chained(&self, organization: Uuid, previous: Uuid) -> Vec<u8> {
        [
            b"be3.escrow.chain.v3".as_slice(),
            organization.as_bytes(),
            self.maker.as_bytes(),
            previous.as_bytes(),
            self.id.as_bytes(),
            &self.epoch.to_be_bytes(),
            &self.public,
            &self.signing,
            &self.head,
        ]
        .concat()
    }
}

impl KeySet {
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.wrapped.is_empty()
    }

    pub fn extend(&mut self, other: KeySet) {
        self.keys.extend(other.keys);
        self.wrapped.extend(other.wrapped);
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the server refused ({}): {}",
            self.code, self.message
        )
    }
}

impl std::error::Error for Error {}

pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_stdvec(message)
}

pub fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}

const HELLO: &[u8] = b"be3 hello\n";

/// What a client's greeting says after its build: the token it signs in with. None signs in
/// with the request's Authorization header instead (`be` behind a proxy that injects it), and
/// is what a one-shot call, which signs nothing in, says.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Greeting {
    pub token: Option<String>,
}

/// What the server's greeting says after its build to a connection of the same build: who
/// the greeting signed it in as, or why it did not, the connection then closed before any call.
/// A one-shot call's answer says nothing (`()`).
pub type Welcome = Result<Authenticated, Error>;

/// A greeting: `HELLO`, the build, then, past a zero byte, what it says. Only the build is
/// read before both ends know they are of one build, so what follows it may change freely.
pub fn hello(build: &str, said: &impl Serialize) -> Vec<u8> {
    let mut frame = [HELLO, build.as_bytes(), &[0]].concat();
    frame.extend(encode(said).unwrap_or_default());
    frame
}

/// A greeting's build, and what it says, to decode with `Hello::said` once the build is
/// known to be ours.
pub struct Hello<'a> {
    pub build: String,
    said: &'a [u8],
}

impl Hello<'_> {
    pub fn said<T: for<'b> Deserialize<'b>>(&self) -> Option<T> {
        decode(self.said).ok()
    }
}

pub fn read_hello(frame: &[u8]) -> Option<Hello<'_>> {
    let rest = frame.strip_prefix(HELLO)?;
    let (build, said) = match rest.iter().position(|byte| *byte == 0) {
        Some(at) => (&rest[..at], &rest[at + 1..]),
        None => (rest, &[][..]),
    };
    Some(Hello {
        build: String::from_utf8(build.to_vec()).ok()?,
        said,
    })
}

/// The case of `Page` a query is answered with, so a read names what it expects.
pub trait FromPage: Sized {
    fn from_page(page: Page) -> Option<Self>;
}

/// The case of `Done` an op is answered with.
pub trait FromDone: Sized {
    fn from_done(done: Done) -> Option<Self>;
}

macro_rules! answers {
    ($trait:ident, $method:ident, $from:ident { $($case:ident => $ty:ty,)* }) => {
        $(impl $trait for $ty {
            fn $method(answer: $from) -> Option<Self> {
                match answer {
                    $from::$case(value) => Some(value),
                    _ => None,
                }
            }
        })*
    };
}

answers!(FromPage, from_page, Page {
    Summaries => Vec<Option<BlockSummary>>,
    Contents => Contents,
    Listed => Listed,
    Tree => ChangedPage,
    History => Vec<HistoryEntry>,
    Log => Vec<Vec<u8>>,
    Access => AccessList,
    Keys => KeyPage,
    KeyWork => KeyWork,
    Revocations => Vec<Revocation>,
    RecoveryKey => RecoveryKey,
    SigningKeys => Vec<SigningKey>,
    Members => Vec<Member>,
    Organizations => Vec<Organization>,
    Invitations => Vec<InvitationRow>,
    Storage => AccountStorage,
    Notices => Vec<AccountNotice>,
    Contacts => Vec<ContactRow>,
    Links => Vec<OwnLink>,
    Link => Link,
    Invitation => InvitationPreview,
    DownloadUrls => Vec<Option<String>>,
    ServerInfo => ServerInfo,
    NotifyModes => Vec<BlockNotifyMode>,
    Session => JoinedSession,
});

answers!(FromDone, from_done, Done {
    Block => BlockSummary,
    Published => PublishOutcome,
    Uploads => Vec<Option<UploadUrl>>,
    Organization => Organization,
    Agent => CreatedAgent,
    Token => String,
});

/// What a `publish` with no commit answers: the block as it is now, or, when its head was not
/// the one expected, that head, as a stale publish with a commit is rejected.
#[derive(Clone, Debug, PartialEq)]
pub enum Described {
    Block(BlockSummary),
    Rejected(Option<be_block_commit::CommitId>),
}

impl FromDone for Described {
    fn from_done(done: Done) -> Option<Self> {
        match done {
            Done::Block(summary) => Some(Self::Block(summary)),
            Done::Published(PublishOutcome::Rejected(head)) => Some(Self::Rejected(head)),
            _ => None,
        }
    }
}

impl FromDone for () {
    fn from_done(done: Done) -> Option<Self> {
        matches!(done, Done::Nothing).then_some(())
    }
}

fn unexpected() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "the server answered with another case than the call asked for",
    )
}

/// Reads `query` in `organization` (`server.read`), as the page case `T` it is answered with.
/// The call starts now, so calls made one after another reach the server in that order.
pub fn read<T: FromPage + Send, C: be_wasm_ops::net::Caller + Clone>(
    caller: &C,
    organization: Option<Uuid>,
    query: Query,
) -> impl Future<Output = Result<Result<T, Error>, be_wasm_ops::OpError>> + Send + 'static + use<T, C>
{
    answered(client::server::read(caller, organization, query, false))
}

/// Reads `query` as `read` does, and with `follow` follows its topic first (`server.read`): what
/// changes after it arrives as `server.events`.
pub fn ask<T: FromPage + Send, C: be_wasm_ops::net::Caller + Clone>(
    caller: &C,
    organization: Option<Uuid>,
    query: Query,
    follow: bool,
) -> impl Future<Output = Result<Result<T, Error>, be_wasm_ops::OpError>> + Send + 'static + use<T, C>
{
    answered(client::server::read(caller, organization, query, follow))
}

fn answered<T: FromPage + Send>(
    pending: impl Future<Output = Result<Result<Page, Error>, be_wasm_ops::OpError>> + Send + 'static,
) -> impl Future<Output = Result<Result<T, Error>, be_wasm_ops::OpError>> + Send + 'static {
    async move {
        Ok(pending
            .await?
            .and_then(|page| T::from_page(page).ok_or_else(unexpected)))
    }
}

/// Writes one op in `organization` (`server.write`), answered as the case `T` it makes.
pub fn write<T: FromDone + Send, C: be_wasm_ops::net::Caller + Clone>(
    caller: &C,
    organization: Option<Uuid>,
    op: Op,
) -> impl Future<Output = Result<Result<T, Error>, be_wasm_ops::OpError>> + Send + 'static + use<T, C>
{
    write_all(caller, organization, vec![op])
}

/// Writes `ops` in `organization` in one write (`server.write`), applied whole or not at all,
/// answered as the case `T` the first makes; what the others make is dropped. A refusal is the
/// refused op's error, whichever op it was.
pub fn write_all<T: FromDone + Send, C: be_wasm_ops::net::Caller + Clone>(
    caller: &C,
    organization: Option<Uuid>,
    ops: Vec<Op>,
) -> impl Future<Output = Result<Result<T, Error>, be_wasm_ops::OpError>> + Send + 'static + use<T, C>
{
    let count = ops.len();
    let pending = client::server::write(caller, organization, ops);
    async move {
        Ok(pending
            .await?
            .map_err(|refused| refused.error)
            .and_then(|done| match (done.len() == count, done.into_iter().next()) {
                (true, Some(done)) => T::from_done(done).ok_or_else(unexpected),
                _ => Err(unexpected()),
            }))
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "op {} of the write: {}", self.index, self.error)
    }
}

impl std::error::Error for WriteError {}

#[cfg(test)]
mod tests;
