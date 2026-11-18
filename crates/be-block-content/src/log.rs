//! A block's signed log as a type sees it (plans/signed_logs.md): content is what anyone with
//! Edit may rewrite; a log is what its authors add, and only they may take back. A type with a
//! log folds its entries, in the log's order, into what it shows (`LogFold`).

use uuid::Uuid;

/// Who signed a log entry as the app's, as far as the device can tell
/// (`be_block_client::Signer`): `Unknown` while it cannot (offline, or unopened), and
/// `Redacted` once its payload is gone, which leaves nothing to check the signature against.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum LogSigner {
    Verified,
    Unverified,
    Changed,
    Replaced,
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
    pub signer: LogSigner,
}

/// How a type's log changes what its content shows, for its description (and, later, search):
/// the log's entries folded in order, each id once, onto the content. The fold never changes
/// the content anyone commits: it is applied to a copy. It is handed the run of entries from
/// the log's start the replica holds, up to the first it lacks; never an entry signed as
/// nobody's (`LogSigner::Bad`), and each other with its verdict, so a type decides what one
/// signed with a replaced key (`Replaced`), not yet told (`Unknown`) or redacted counts for.
pub trait LogFold: crate::BlockContent {
    /// Whether what the content shows depends on its log at all. A type whose content shows
    /// nothing of its log (chat: its children are its content's) says false, and its replica
    /// neither reads the log nor keeps it: its editors read what they show. One that folds has
    /// its replica read the log to its end, a page at a time, so the fold is never cut short.
    const FOLDS: bool = true;

    fn fold(&mut self, item: &LogItem);
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
