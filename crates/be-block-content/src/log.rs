//! A block's signed log as a type sees it (plans/signed_logs.md): content is what anyone with
//! Edit may rewrite; a log is what its authors add, and only they may take back. A type with a
//! log folds its entries, in the log's order, into what it shows (`LogFold`).

use uuid::Uuid;

/// Who signed a log entry as the app's, as far as this device can tell. Verifying a signature
/// answers one of the first five; a log entry is `Unknown` while that cannot be told (offline,
/// or its payload unopened), and `Redacted` once its payload is gone, which leaves nothing to
/// check the signature against.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Signer {
    /// The account's key, verified through a contact link or a key change sealed with the one
    /// before (or this account's own).
    Verified,
    /// The key the server named the first time this device asked, pinned since and never
    /// verified.
    Unverified,
    /// A key other than the account's current one: the server names one that is not the one
    /// this device pinned, or a contact's key a new phrase replaced. The account took a new
    /// phrase, or the server is lying, or someone holds the old phrase.
    Changed,
    /// A contact's key it had replaced before the server took the record, after this device
    /// learned of the new one: one of the account's devices not yet told of its new phrase, a
    /// send held offline until after it, or someone holding the old phrase. Not proof of a
    /// forgery, so not `Bad`, but louder than `Changed`.
    Replaced,
    /// No key this device knows the account by, nor one the server names, made the signature:
    /// a forgery, or bytes changed since they were signed.
    Bad,
    Unknown,
    Redacted,
}

/// An entry of a block's signed log: its place, who appended it and when the server took it
/// (neither of which its author chooses), and the bytes the app appended.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LogItem {
    pub seq: u64,
    pub id: Uuid,
    pub author: Uuid,
    pub arrival: i64,
    /// None once redacted, or while no key the device holds opens it.
    #[serde(with = "serde_bytes")]
    pub payload: Option<Vec<u8>>,
    pub signer: Signer,
    /// For history an import brought from another block: where it was written. Its author and
    /// arrival are then the file's word, as written there.
    pub imported: Option<LogImport>,
}

/// Where an imported entry of a log was written: the block, its place in that block's log, and
/// who imported it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LogImport {
    pub block: Uuid,
    pub seq: u64,
    pub importer: Uuid,
}

/// How a type's log changes what its content shows, for its description (and, later, search):
/// the log's entries folded in order, each id once, onto the content. The fold never changes
/// the content anyone commits: it is applied to a copy. It is handed the run of entries from
/// the log's start the replica holds, up to the first it lacks; never an entry signed as
/// nobody's (`Signer::Bad`), and each other with its verdict, so a type decides what one
/// signed with a replaced key (`Replaced`), not yet told (`Unknown`) or redacted counts for.
pub trait LogFold: crate::BlockContent {
    /// Whether what the content shows depends on its log at all. A type whose content shows
    /// nothing of its log (chat: its children are its content's) says false, and its replica
    /// neither reads the log nor keeps it: its editors read what they show. One that folds has
    /// its replica read the log to its end, a page at a time, so the fold is never cut short.
    const FOLDS: bool = true;

    fn fold(&mut self, item: &LogItem);

    /// What `item` changes in the type's searchable text, if anything: the document of the
    /// entry at `LogSearch::place` (its own place, for an entry that adds one) says `text` now,
    /// or is gone. The host applies a change to another entry's document only when that entry
    /// has the same author, so nobody changes what someone else wrote.
    fn search(_item: &LogItem) -> Option<LogSearch> {
        None
    }
}

/// What an entry of a block's signed log changes in its searchable text (`LogFold::search`).
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LogSearch {
    pub place: u64,
    /// None: the document is gone.
    pub text: Option<String>,
}

/// What goes with an entry an editor appends, which the device keeps with the entry until the
/// server takes it and then does, so neither happens for an entry the server never takes, nor
/// is lost to a restart before it does.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Along {
    /// What the server pushes to the block's members who are away: several taken at once go
    /// as one (`Announcement::several`).
    pub announcement: Option<crate::Announcement>,
    /// The place of an entry of the block's log the new one takes back (a deletion's message),
    /// redacted: the entry's author's, or a manager's, as the server checks.
    pub redacts: Option<u64>,
}

/// Where an entry appended to a block's signed log went: its id, and its place and arrival once
/// the server took it, none while it waits on the device for the server.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LogAppended {
    pub id: Uuid,
    pub place: Option<(u64, i64)>,
}

/// Why an entry was not appended to a block's signed log: `id` names one the server refused,
/// which the device keeps until let go (`LogPage::refused`); none for one it never made.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LogNotAppended {
    pub id: Option<Uuid>,
    pub why: String,
}

/// A run of a block's signed log read: the log's length as the device verified it, the entries
/// asked for, the entries the device appended that the server has not taken yet, and those the
/// server refused that the device keeps until let go.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LogPage {
    pub size: u64,
    pub items: Vec<LogItem>,
    pub kept: Vec<KeptEntry>,
    pub refused: Vec<RefusedEntry>,
}

/// An entry this device appended that waits for the server: its id and the bytes the app
/// appended.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct KeptEntry {
    pub id: Uuid,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

/// An entry this device appended that the server refused, kept until let go: its id, the bytes
/// the app appended and why.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RefusedEntry {
    pub id: Uuid,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    pub why: String,
}
