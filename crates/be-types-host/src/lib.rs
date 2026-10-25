use std::sync::{Arc};

use wasi_threads::Mutex;

use be_client::ClientError;
use be_commit::CommitId;
use be_store::ObjectStore;
use block_type_api::{Description, EditorId, Status};
use uuid::Uuid;

pub mod modules;
pub mod replicas;

pub use block_type_api::{Plain, PlainFile};
pub use modules::{
    Kind, Modules, Source, in_process, is_known, kind_of, now_millis, plain_forms_revision,
    plain_of, started, tag_of,
};
pub use replicas::{Ctx, HOST_EDITOR, Replicas};

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

    fn report(shared: &Arc<Mutex<Self>>, message: String);

    fn set_locked(shared: &Arc<Mutex<Self>>, block: Uuid);

    fn owned(shared: &Arc<Mutex<Self>>, block: Uuid) -> bool;

    fn reshaped(shared: &Arc<Mutex<Self>>, block: Uuid, schema: &str) -> bool;

    fn reset_notice();

    fn known_head(shared: &Arc<Mutex<Self>>, block: Uuid) -> Option<CommitId>;

    fn to_editor(shared: &Arc<Mutex<Self>>, block: Uuid, editor: EditorId, bytes: Vec<u8>) -> bool;

    fn content(shared: &Arc<Mutex<Self>>, block: Uuid, content_type: Uuid, bytes: Vec<u8>);

    fn released(shared: &Arc<Mutex<Self>>, block: Uuid);

    fn status(shared: &Arc<Mutex<Self>>, block: Uuid, status: &Status);

    fn described(
        shared: &Arc<Mutex<Self>>,
        block: Uuid,
        references: Vec<Uuid>,
        description: Description,
    );

    fn history(shared: &Arc<Mutex<Self>>, block: Uuid, can_undo: bool, can_redo: bool);

    fn conflict(shared: &Arc<Mutex<Self>>, block: Uuid, conflict: Conflict);

    fn counted(shared: &Arc<Mutex<Self>>, unsealed: usize, owing: usize);
}

#[cfg(test)]
mod tests;
