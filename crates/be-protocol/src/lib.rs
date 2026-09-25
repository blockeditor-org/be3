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

impl WorkspaceRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Administrator => "Administrator",
            Self::Editor => "Editor",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub role: WorkspaceRole,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceInvitation {
    pub id: Uuid,
    pub workspace: Uuid,
    pub workspace_name: String,
    pub email: String,
    pub role: WorkspaceRole,
    pub invited_by: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlockSummary {
    pub id: Uuid,
    pub content_type: Uuid,
    pub author: Uuid,
    pub parent: BlockParent,
    pub head: Option<CommitId>,
    pub access: Access,
    pub references: Vec<Uuid>,
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,
}

pub type ClientId = u64;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionState {
    pub generation: u64,
    pub owner: Option<ClientId>,
    pub participants: Vec<ClientId>,
    pub clean_at: Option<CommitId>,
}

impl SessionState {
    pub fn is_owner(&self, client: ClientId) -> bool {
        self.owner == Some(client)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccessEntry {
    pub account: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: WorkspaceRole,
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
    RegistrationDisabled,
    InvitationNotFound,
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
    Adopt {
        request: u64,
    },
    Logout {
        request: u64,
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
    Invite {
        request: u64,
        workspace: Uuid,
        email: String,
        role: WorkspaceRole,
    },
    ListInvitations {
        request: u64,
    },
    RespondInvitation {
        request: u64,
        invitation: Uuid,
        accept: bool,
    },

    PutObject {
        request: u64,
        #[serde(with = "serde_bytes")]
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
        #[serde(with = "serde_bytes")]
        metadata: Vec<u8>,
    },
    ListBlocks {
        request: u64,
    },
    SetMetadata {
        request: u64,
        block: Uuid,
        #[serde(with = "serde_bytes")]
        metadata: Vec<u8>,
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

    JoinSession {
        request: u64,
        block: Uuid,
    },
    LeaveSession {
        request: u64,
        block: Uuid,
    },
    ClaimOwnership {
        request: u64,
        block: Uuid,
        generation: u64,
    },
    Heartbeat {
        request: u64,
        block: Uuid,
        generation: u64,
        clean_at: Option<CommitId>,
    },
    Relay {
        request: u64,
        block: Uuid,
        to: Option<ClientId>,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
}

impl ClientMessage {
    pub fn request(&self) -> u64 {
        match self {
            Self::Register { request, .. }
            | Self::Login { request, .. }
            | Self::Authenticate { request, .. }
            | Self::Adopt { request }
            | Self::Logout { request }
            | Self::Invite { request, .. }
            | Self::ListInvitations { request }
            | Self::RespondInvitation { request, .. }
            | Self::ListWorkspaces { request }
            | Self::CreateWorkspace { request, .. }
            | Self::OpenWorkspace { request, .. }
            | Self::PutObject { request, .. }
            | Self::GetObject { request, .. }
            | Self::GetObjectRange { request, .. }
            | Self::MissingObjects { request, .. }
            | Self::CreateBlock { request, .. }
            | Self::ListBlocks { request }
            | Self::SetMetadata { request, .. }
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
            | Self::Unwatch { request, .. }
            | Self::JoinSession { request, .. }
            | Self::LeaveSession { request, .. }
            | Self::ClaimOwnership { request, .. }
            | Self::Heartbeat { request, .. }
            | Self::Relay { request, .. } => *request,
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
        email: String,
        display_name: String,
        token: String,
    },
    Workspaces {
        request: u64,
        workspaces: Vec<Workspace>,
    },
    Invitations {
        request: u64,
        invitations: Vec<WorkspaceInvitation>,
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
        #[serde(with = "serde_bytes")]
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
    Session {
        request: u64,
        block: Uuid,
        client: ClientId,
        state: SessionState,
    },
    HeadChanged {
        block: Uuid,
        head: CommitId,
        author: Uuid,
    },
    BlockDeleted {
        block: Uuid,
    },
    BlockChanged {
        block: BlockSummary,
    },
    BlockRemoved {
        block: Uuid,
    },
    SessionChanged {
        block: Uuid,
        state: SessionState,
    },
    Relayed {
        block: Uuid,
        from: ClientId,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
}

impl ServerMessage {
    pub fn request(&self) -> Option<u64> {
        match self {
            Self::Ok { request }
            | Self::Failed { request, .. }
            | Self::Authenticated { request, .. }
            | Self::Workspaces { request, .. }
            | Self::Invitations { request, .. }
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
            | Self::AccessList { request, .. }
            | Self::Session { request, .. } => Some(*request),
            Self::HeadChanged { .. }
            | Self::BlockDeleted { .. }
            | Self::BlockChanged { .. }
            | Self::BlockRemoved { .. }
            | Self::SessionChanged { .. }
            | Self::Relayed { .. } => None,
        }
    }
}

pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_stdvec(message)
}

pub fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}
