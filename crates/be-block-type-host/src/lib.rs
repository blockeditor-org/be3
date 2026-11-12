use std::sync::Arc;

use be_wasm_threads::Mutex;

use be_block_client::ClientError;
use be_block_commit::CommitId;
use be_block_store::ObjectStore;
use be_sdk_type_api::{Description, EditorId, Status};
use uuid::Uuid;

pub mod modules;
pub mod replicas;

pub use be_sdk_type_api::{Plain, PlainFile};
pub use modules::{
    Kind, Modules, OUTSIDE_REQUEST, PartRead, Scope, Source, Under, in_process, is_known, kind_of,
    kinds_revision, plain_of, started, tag_of,
};
pub use replicas::{Ctx, EARLY_OPEN, HOST_EDITOR, Merged, NotSearched, Replicas, Searched};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Conflict {
    pub ours: CommitId,
    pub theirs: CommitId,
    pub count: usize,
}

pub trait Host: Sized + 'static {
    type Store: ObjectStore + 'static;

    fn spawn(task: impl Future<Output = ()> + 'static);

    fn wake();

    fn record(shared: &Arc<Mutex<Self>>, error: ClientError);

    /// Records what went wrong sending to the server, such as a refused append to a session's
    /// journal: unlike `record`, it stops being news once the device is in step again.
    fn record_sync(shared: &Arc<Mutex<Self>>, error: ClientError);

    fn report(shared: &Arc<Mutex<Self>>, block: Option<Uuid>, message: String);

    fn set_locked(shared: &Arc<Mutex<Self>>, block: Uuid);

    fn owned(shared: &Arc<Mutex<Self>>, block: Uuid) -> bool;

    fn reshaped(shared: &Arc<Mutex<Self>>, block: Uuid, schema: &str) -> bool;

    fn reset_notice();

    fn known_head(shared: &Arc<Mutex<Self>>, block: Uuid) -> Option<CommitId>;

    fn to_editor(shared: &Arc<Mutex<Self>>, block: Uuid, editor: EditorId, bytes: Vec<u8>) -> bool;

    fn content(shared: &Arc<Mutex<Self>>, block: Uuid, content_type: Uuid, bytes: Vec<u8>);

    fn released(shared: &Arc<Mutex<Self>>, block: Uuid);

    /// Also when only `status.undo` changed.
    fn status(shared: &Arc<Mutex<Self>>, block: Uuid, status: &Status);

    fn described(
        shared: &Arc<Mutex<Self>>,
        block: Uuid,
        references: Vec<Uuid>,
        description: Description,
    );

    fn conflict(shared: &Arc<Mutex<Self>>, block: Uuid, conflict: Conflict);

    fn counted(shared: &Arc<Mutex<Self>>, unsealed: usize, owing: usize, undurable: usize);

    fn leaving(shared: &Arc<Mutex<Self>>, leaving: bool);
}

#[cfg(test)]
mod tests;
