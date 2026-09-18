use std::fmt;

use be_commit::CommitId;
use be_graph::{Access, BlockParent};
use be_store::Hash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;

pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkspaceRole {
    Administrator,
    Editor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub role: WorkspaceRole,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlockSummary {
    pub id: Uuid,
    pub content_type: Uuid,
    pub author: Uuid,
    pub parent: BlockParent,
    pub head: Option<CommitId>,
    pub access: Access,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccessEntry {
    pub account: Uuid,
    pub email: String,
    pub display_name: String,
    pub granted: Option<Access>,
    pub effective: Access,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ErrorCode {
    NotAuthenticated,
    InvalidCredentials,
    EmailAlreadyRegistered,
    InvalidRequest,
    PermissionDenied,
    BlockNotFound,
    BlockAlreadyExists,
    ObjectNotFound,
    ObjectTooLarge,
    ParentCycle,
    Storage,
    WorkspaceNotFound,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ClientMessage {
    Register {
        request: u64,
        email: String,
        display_name: String,
        password: String,
    },
    Login {
        request: u64,
        email: String,
        password: String,
    },
    Authenticate {
        request: u64,
        token: String,
    },
    ListWorkspaces {
        request: u64,
    },
    CreateWorkspace {
        request: u64,
        name: String,
    },
    OpenWorkspace {
        request: u64,
        workspace: Uuid,
    },

    PutObject {
        request: u64,
        bytes: Vec<u8>,
    },
    GetObject {
        request: u64,
        hash: Hash,
    },
    GetObjectRange {
        request: u64,
        hash: Hash,
        offset: u64,
        length: u64,
    },
    MissingObjects {
        request: u64,
        hashes: Vec<Hash>,
    },

    CreateBlock {
        request: u64,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
    },
    Publish {
        request: u64,
        block: Uuid,
        commit: CommitId,
        expected: Option<CommitId>,
        chunks: Vec<Hash>,
        time: i64,
        pinned: bool,
        references_added: Vec<Uuid>,
        references_removed: Vec<Uuid>,
    },
    ReadBlock {
        request: u64,
        block: Uuid,
    },
    SetParent {
        request: u64,
        block: Uuid,
        parent: BlockParent,
    },
    ListChildren {
        request: u64,
        parent: BlockParent,
    },
    ListBackrefs {
        request: u64,
        block: Uuid,
    },
    ListHistory {
        request: u64,
        block: Uuid,
    },
    PruneHistory {
        request: u64,
        block: Uuid,
        drop: Vec<CommitId>,
    },
    DeleteBlock {
        request: u64,
        block: Uuid,
    },
    CollectDetached {
        request: u64,
    },
    SetAccess {
        request: u64,
        block: Uuid,
        account: Uuid,
        access: Access,
    },
    ListAccess {
        request: u64,
        block: Uuid,
    },
    Watch {
        request: u64,
        block: Uuid,
    },
    Unwatch {
        request: u64,
        block: Uuid,
    },
}

impl ClientMessage {
    pub fn request(&self) -> u64 {
        match self {
            Self::Register { request, .. }
            | Self::Login { request, .. }
            | Self::Authenticate { request, .. }
            | Self::ListWorkspaces { request }
            | Self::CreateWorkspace { request, .. }
            | Self::OpenWorkspace { request, .. }
            | Self::PutObject { request, .. }
            | Self::GetObject { request, .. }
            | Self::GetObjectRange { request, .. }
            | Self::MissingObjects { request, .. }
            | Self::CreateBlock { request, .. }
            | Self::Publish { request, .. }
            | Self::ReadBlock { request, .. }
            | Self::SetParent { request, .. }
            | Self::ListChildren { request, .. }
            | Self::ListBackrefs { request, .. }
            | Self::ListHistory { request, .. }
            | Self::PruneHistory { request, .. }
            | Self::DeleteBlock { request, .. }
            | Self::CollectDetached { request }
            | Self::SetAccess { request, .. }
            | Self::ListAccess { request, .. }
            | Self::Watch { request, .. }
            | Self::Unwatch { request, .. } => *request,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryEntry {
    pub commit: CommitId,
    pub time: i64,
    pub pinned: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ServerMessage {
    Ok {
        request: u64,
    },
    Failed {
        request: u64,
        code: ErrorCode,
        message: String,
    },
    Authenticated {
        request: u64,
        account: Uuid,
        display_name: String,
        token: String,
    },
    Workspaces {
        request: u64,
        workspaces: Vec<Workspace>,
    },
    WorkspaceOpened {
        request: u64,
        workspace: Uuid,
        role: WorkspaceRole,
    },
    Stored {
        request: u64,
        hash: Hash,
    },
    Object {
        request: u64,
        bytes: Option<Vec<u8>>,
    },
    Missing {
        request: u64,
        hashes: Vec<Hash>,
    },
    Block {
        request: u64,
        block: BlockSummary,
    },
    Blocks {
        request: u64,
        blocks: Vec<BlockSummary>,
    },
    Published {
        request: u64,
        head: CommitId,
    },
    Rejected {
        request: u64,
        head: Option<CommitId>,
    },
    History {
        request: u64,
        entries: Vec<HistoryEntry>,
    },
    Collected {
        request: u64,
        blocks: Vec<Uuid>,
        objects: usize,
    },
    AccessList {
        request: u64,
        entries: Vec<AccessEntry>,
    },
    HeadChanged {
        block: Uuid,
        head: CommitId,
        author: Uuid,
    },
    BlockDeleted {
        block: Uuid,
    },
}

impl ServerMessage {
    pub fn request(&self) -> Option<u64> {
        match self {
            Self::Ok { request }
            | Self::Failed { request, .. }
            | Self::Authenticated { request, .. }
            | Self::Workspaces { request, .. }
            | Self::WorkspaceOpened { request, .. }
            | Self::Stored { request, .. }
            | Self::Object { request, .. }
            | Self::Missing { request, .. }
            | Self::Block { request, .. }
            | Self::Blocks { request, .. }
            | Self::Published { request, .. }
            | Self::Rejected { request, .. }
            | Self::History { request, .. }
            | Self::Collected { request, .. }
            | Self::AccessList { request, .. } => Some(*request),
            Self::HeadChanged { .. } | Self::BlockDeleted { .. } => None,
        }
    }
}

pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_stdvec(message)
}

pub fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}
