use std::sync::Arc;

use std::sync::Mutex;

use be_block_client::ClientError;
use be_block_commit::CommitId;
use be_block_graph::BlockParent;
use be_block_store::ObjectStore;
use be_sdk_type_api::{Description, EditorId, Status};
use uuid::Uuid;

pub mod modules;
pub mod replicas;

pub use be_sdk_type_api::{Plain, PlainFile};
pub use modules::{
    Kind, Modules, OUTSIDE_REQUEST, PartRead, Scope, Source, Under, in_process, is_known, kind_of,
    known, known_revision, plain_of, started, tag_of,
};
pub use replicas::{
    Ctx, EARLY_OPEN, HOST_EDITOR, LOG_SEARCHED, LogChange, LogSearched, Merged, NotSearched, Owing,
    Replicas, Searched,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Conflict {
    pub ours: CommitId,
    pub theirs: CommitId,
    pub count: usize,
}

/// What a replica tells and asks the program holding it. The methods with bodies have neutral
/// defaults (nothing told, nothing known), which a host overrides when it shows or keeps what
/// they tell.
pub trait Host: Sized + 'static {
    type Store: ObjectStore + 'static;

    fn spawn(task: impl Future<Output = ()> + 'static);

    fn wake();

    fn record(shared: &Arc<Mutex<Self>>, error: ClientError);

    /// Records what went wrong sending to the server, such as a refused append to a session's
    /// journal: unlike `record`, it stops being news once the device is in step again.
    fn record_sync(shared: &Arc<Mutex<Self>>, error: ClientError);

    fn report(shared: &Arc<Mutex<Self>>, block: Option<Uuid>, message: String);

    /// Whether `block` waits for a key this device does not hold: set as a read meets one,
    /// cleared by `Replicas::unlock` as keys arrive.
    fn set_locked(shared: &Arc<Mutex<Self>>, block: Uuid, locked: bool);

    /// Whether `block`'s editors are attached read-only because its local edits were discarded
    /// and no join has said since that this account may edit it (`Peer::discarded`): told as a
    /// replica opens, and as each join answers once the replica it opens has its editors
    /// attached, so a `false` means its editors' edits are taken.
    fn set_marked(_shared: &Arc<Mutex<Self>>, _block: Uuid, _marked: bool) {}

    /// Whether this device holds `block` as its own (its settings), so a replica resets it when
    /// `reshaped` says its schema changed instead of joining it. None by default.
    fn owned(_shared: &Arc<Mutex<Self>>, _block: Uuid) -> bool {
        false
    }

    /// Whether the schema `block` was stamped with differs from `schema`. Asked only of an
    /// `owned` block.
    fn reshaped(_shared: &Arc<Mutex<Self>>, _block: Uuid, _schema: &str) -> bool {
        false
    }

    /// Tells the person that an `owned` block was reset.
    fn reset_notice() {}

    /// A head of `block` the host heard of beyond the server's, which an opening replica merges
    /// in. None by default.
    fn known_head(_shared: &Arc<Mutex<Self>>, _block: Uuid) -> Option<CommitId> {
        None
    }

    fn to_editor(shared: &Arc<Mutex<Self>>, block: Uuid, editor: EditorId, bytes: Vec<u8>) -> bool;

    /// The content as it now reads: a host that mirrors content (to read it without a replica,
    /// or to index it for search) keeps it from here.
    fn content(_shared: &Arc<Mutex<Self>>, _block: Uuid, _content_type: Uuid, _bytes: Vec<u8>) {}

    fn released(shared: &Arc<Mutex<Self>>, block: Uuid);

    /// Also when only `status.undo` changed.
    fn status(_shared: &Arc<Mutex<Self>>, _block: Uuid, _status: &Status) {}

    /// The block's references and description as its content gives them: a host that shows
    /// them (names, the graph) or searches them keeps them from here.
    fn described(
        _shared: &Arc<Mutex<Self>>,
        _block: Uuid,
        _references: Vec<Uuid>,
        _description: Description,
    ) {
    }

    fn conflict(_shared: &Arc<Mutex<Self>>, _block: Uuid, _conflict: Conflict) {}

    /// How many open replicas have edits not yet committed, which owe a seal (`Owing`), and how
    /// many hold edits not yet durable (`Replicas::counts`). A host that can exit while some are
    /// not durable must keep this and wait on them before it stops, or those edits are lost: the
    /// default forgets it, which only a host that never stops with edits open may.
    fn counted(
        _shared: &Arc<Mutex<Self>>,
        _unsealed: Vec<Owing>,
        _owing: Vec<Owing>,
        _undurable: Vec<Owing>,
    ) {
    }

    fn leaving(_shared: &Arc<Mutex<Self>>, _leaving: bool) {}

    /// What the worker waits on its modules for while it awaits one's answer (`Replicas::wait`),
    /// each as "<block>: <what was asked>", empty once answered: for a host to name when it stays
    /// busy.
    fn asking(_shared: &Arc<Mutex<Self>>, _asking: Vec<String>) {}

    /// A replica made `block` on the server under `parent` itself (`Peer::ensure_type`), not
    /// through the outbox: what the host lists of its place reads it again, as after a drain
    /// sends a creation.
    fn created(_shared: &Arc<Mutex<Self>>, _block: Uuid, _parent: BlockParent) {}
}

#[cfg(test)]
mod tests;
