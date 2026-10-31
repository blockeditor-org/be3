use std::fmt;

use be_commit::CommitId;
use be_graph::{Access, BlockParent};
use be_store::Hash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use serde_bytes::ByteBuf;

be_ops_bindgen::generate!({
    path: "wit",
    world: "client",
    deps: ["../be-ops/wit"],
    with: {
        "uuid": uuid::Uuid,
        "hash": be_store::Hash,
        "commit-id": be_commit::CommitId,
        "public-key": [u8; 32],
    },
    copy: ["uuid", "hash", "commit-id", "public-key"],
    default: ["key-set", "key-work", "session-state", "access-list", "organization-kind"],
    inline: ["block-removed", "head-moved", "joined", "relayed"],
});

pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum OrganizationRole {
    Administrator,
    Editor,
}

impl OrganizationRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Administrator => "Administrator",
            Self::Editor => "Editor",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum OrganizationKind {
    #[default]
    Company,
    Personal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub kind: OrganizationKind,
    pub role: OrganizationRole,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OrganizationInvitation {
    pub id: Uuid,
    pub organization: Uuid,
    pub organization_name: String,
    pub email: String,
    pub role: OrganizationRole,
    pub invited_by: Uuid,
    #[serde(with = "serde_bytes")]
    pub sealed: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub escrow: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PendingGrant {
    pub organization: Uuid,
    pub block: Uuid,
    pub access: Access,
    pub key: Uuid,
    #[serde(with = "serde_bytes")]
    pub sealed: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContactLink {
    pub id: Uuid,
    pub owner: Uuid,
    #[serde(with = "serde_bytes")]
    pub card: Vec<u8>,
    pub grant: Option<PendingGrant>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnContactLink {
    pub id: Uuid,
    #[serde(with = "serde_bytes")]
    pub code: Vec<u8>,
    pub accepter: Option<Uuid>,
    #[serde(with = "serde_bytes")]
    pub card: Vec<u8>,
    pub grant: Option<PendingGrant>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContactRow {
    pub account: Uuid,
    pub email: String,
    pub display_name: String,
    #[serde(with = "serde_bytes")]
    pub sealed: Vec<u8>,
    pub changes: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeyChange {
    pub contact: Uuid,
    #[serde(with = "serde_bytes")]
    pub sealed: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum OwnKind {
    Contact,
    Link,
    Escrow,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnSealed {
    pub kind: OwnKind,
    pub id: Uuid,
    #[serde(with = "serde_bytes")]
    pub sealed: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Listing {
    Children(BlockParent),
    Deleted,
    Shared,
    Backrefs(Uuid),
    Block(Uuid),
    References(Uuid),
    OpenSettings(Uuid),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlockSummary {
    pub id: Uuid,
    pub content_type: Uuid,
    pub access: Access,
    pub author: Option<Uuid>,
    pub parent: Option<BlockParent>,
    pub head: Option<CommitId>,
    pub references: Vec<Uuid>,
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,
    pub version: Option<u64>,
    pub key: Option<Uuid>,
    pub scope: Option<Uuid>,
    pub revision: u64,
    pub held: bool,
    #[serde(default)]
    pub opens_inside: bool,
    pub sets_access: bool,
}

impl BlockSummary {
    pub fn no_access(id: Uuid, content_type: Uuid, access: Access) -> Self {
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
            revision: 0,
            held: true,
            opens_inside: false,
            sets_access: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum KeyKind {
    Data,
    Scope,
    Escrow,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeyRow {
    pub id: Uuid,
    pub block: Uuid,
    pub kind: KeyKind,
    pub epoch: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum WrappedBy {
    Key(Uuid),
    Account(Uuid),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WrappedKey {
    pub key: Uuid,
    pub wrapped_by: WrappedBy,
    pub sender: Uuid,
    #[serde(with = "serde_bytes")]
    pub sealed: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EscrowKey {
    pub id: Uuid,
    pub epoch: u64,
    pub public: [u8; 32],
    pub signing: [u8; 32],
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

    pub fn chained(&self, organization: Uuid, previous: Uuid) -> Vec<u8> {
        [
            b"be3.escrow.chain.v1".as_slice(),
            organization.as_bytes(),
            previous.as_bytes(),
            self.id.as_bytes(),
            &self.epoch.to_be_bytes(),
            &self.public,
            &self.signing,
        ]
        .concat()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Unsealed {
    pub key: Uuid,
    pub account: Uuid,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeyWork {
    pub rotations: Vec<Uuid>,
    pub escrow_rotation: bool,
    pub unsealed: Vec<Unsealed>,
    pub unescrowed: Vec<Uuid>,
    pub escrows: Vec<EscrowKey>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Member {
    pub account: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: OrganizationRole,
    pub agent: bool,
    pub contact: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeySet {
    pub keys: Vec<KeyRow>,
    pub wrapped: Vec<WrappedKey>,
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
    pub role: OrganizationRole,
    pub author: bool,
    pub granted: Option<Access>,
    pub inherited: Access,
    pub effective: Access,
    pub source: Option<Uuid>,
    pub contact: bool,
    pub waiting: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccessList {
    pub entries: Vec<AccessEntry>,
    pub company: bool,
    pub set_inside: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ServerAccount {
    pub account: Uuid,
    pub public: [u8; 32],
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
    OrganizationNotFound,
    InvitationNotFound,
    LoginDisabled,
    BlockHasContent,
    KeyMissing,
    NotAContact,
    LinkNotFound,
    LastAdministrator,
    PolicyUnreadable,
    PolicyRefused,
    OverQuota,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ClientMessage {
    BeginBrowserLogin {
        request: u64,
    },
    FinishBrowserLogin {
        request: u64,
        code: String,
        secret: String,
    },
    CreateLocalProfile {
        request: u64,
        display_name: String,
    },
    Authenticate {
        request: u64,
        token: String,
    },
    AuthenticateHeader {
        request: u64,
    },
    Logout {
        request: u64,
    },
    ListOrganizations {
        request: u64,
    },
    CreateOrganization {
        request: u64,
        name: String,
    },
    OpenOrganization {
        request: u64,
        organization: Uuid,
    },
    Invite {
        request: u64,
        invitation: Uuid,
        organization: Uuid,
        email: String,
        role: OrganizationRole,
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
        #[serde(with = "serde_bytes")]
        code: Vec<u8>,
        #[serde(with = "serde_bytes")]
        escrow: Vec<u8>,
    },
    GetInvitation {
        request: u64,
        invitation: Uuid,
    },
    AcceptInvitation {
        request: u64,
        invitation: Uuid,
        #[serde(with = "serde_bytes")]
        card: Vec<u8>,
        #[serde(with = "serde_bytes")]
        contact: Vec<u8>,
        #[serde(with = "serde_bytes")]
        escrow: Vec<u8>,
    },
    CreateContactLink {
        request: u64,
        link: Uuid,
        #[serde(with = "serde_bytes")]
        card: Vec<u8>,
        #[serde(with = "serde_bytes")]
        code: Vec<u8>,
        grant: Option<PendingGrant>,
    },
    GetContactLink {
        request: u64,
        link: Uuid,
    },
    AcceptContactLink {
        request: u64,
        link: Uuid,
        #[serde(with = "serde_bytes")]
        card: Vec<u8>,
        #[serde(with = "serde_bytes")]
        contact: Vec<u8>,
        #[serde(with = "serde_bytes")]
        key: Vec<u8>,
    },
    ListContactLinks {
        request: u64,
    },
    DeleteContactLink {
        request: u64,
        link: Uuid,
    },
    PutContact {
        request: u64,
        contact: Uuid,
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    ListContacts {
        request: u64,
    },
    RemoveContact {
        request: u64,
        account: Uuid,
    },

    PutObjects {
        request: u64,
        objects: Vec<ByteBuf>,
    },
    GetObject {
        request: u64,
        hash: Hash,
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
        references: Vec<Uuid>,
        key: Uuid,
        keys: KeySet,
    },
    SetMetadata {
        request: u64,
        block: Uuid,
        #[serde(with = "serde_bytes")]
        metadata: Vec<u8>,
        references: Vec<Uuid>,
    },
    Publish {
        request: u64,
        block: Uuid,
        commit: CommitId,
        expected: Option<CommitId>,
        parents: Vec<CommitId>,
        chunks: Vec<Hash>,
        time: i64,
        pinned: bool,
        references: Vec<Uuid>,
        #[serde(with = "serde_bytes")]
        metadata: Vec<u8>,
        objects: Vec<ByteBuf>,
    },
    HoldObjects {
        request: u64,
        block: Uuid,
        objects: Vec<Hash>,
    },
    ReadBlock {
        request: u64,
        block: Uuid,
    },
    ReadContents {
        request: u64,
        blocks: Vec<Uuid>,
        held: Vec<CommitId>,
    },
    SetParent {
        request: u64,
        block: Uuid,
        parent: BlockParent,
        keys: KeySet,
    },
    ListShared {
        request: u64,
    },
    ListChildren {
        request: u64,
        parent: BlockParent,
    },
    ListDeleted {
        request: u64,
    },
    ListBackrefs {
        request: u64,
        block: Uuid,
    },
    ListReferences {
        request: u64,
        block: Uuid,
    },
    ListOpenSettings {
        request: u64,
        block: Uuid,
    },
    Unlist {
        request: u64,
        listing: Listing,
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
    GetKeys {
        request: u64,
        blocks: Option<Vec<Uuid>>,
    },
    PutKeys {
        request: u64,
        keys: KeySet,
    },
    GetRecoveryKey {
        request: u64,
    },
    SetRecoveryKey {
        request: u64,
        public: [u8; 32],
        sealed: Vec<SealedKey>,
        own: Vec<OwnSealed>,
        changes: Vec<KeyChange>,
    },
    ResealKeys {
        request: u64,
        keys: Vec<SealedKey>,
    },
    Pair {
        request: u64,
        to: Option<ClientId>,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
    CreateAgent {
        request: u64,
        organization: Uuid,
        name: String,
        recovery: [u8; 32],
        #[serde(with = "serde_bytes")]
        contact: Vec<u8>,
        #[serde(with = "serde_bytes")]
        escrow: Vec<u8>,
    },
    ListAgents {
        request: u64,
        organization: Uuid,
    },
    RemoveAgent {
        request: u64,
        organization: Uuid,
        account: Uuid,
    },
    GetKeyWork {
        request: u64,
    },
    Rotate {
        request: u64,
        roots: Vec<Uuid>,
        keys: KeySet,
    },
    CreateEscrow {
        request: u64,
        escrow: EscrowKey,
        keys: KeySet,
    },
    PinEscrow {
        request: u64,
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    ListMembers {
        request: u64,
    },
    SetRole {
        request: u64,
        account: Uuid,
        role: OrganizationRole,
    },
    RemoveMember {
        request: u64,
        account: Uuid,
    },
    Ping {
        request: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Agent {
    pub account: Uuid,
    pub name: String,
    pub email: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SealedKey {
    pub organization: Uuid,
    pub key: Uuid,
    pub sender: Uuid,
    #[serde(with = "serde_bytes")]
    pub sealed: Vec<u8>,
}

impl ClientMessage {
    pub fn request(&self) -> u64 {
        match self {
            Self::BeginBrowserLogin { request }
            | Self::FinishBrowserLogin { request, .. }
            | Self::CreateLocalProfile { request, .. }
            | Self::Authenticate { request, .. }
            | Self::AuthenticateHeader { request }
            | Self::Logout { request }
            | Self::Invite { request, .. }
            | Self::GetInvitation { request, .. }
            | Self::AcceptInvitation { request, .. }
            | Self::CreateContactLink { request, .. }
            | Self::GetContactLink { request, .. }
            | Self::AcceptContactLink { request, .. }
            | Self::ListContactLinks { request }
            | Self::DeleteContactLink { request, .. }
            | Self::PutContact { request, .. }
            | Self::ListContacts { request }
            | Self::RemoveContact { request, .. }
            | Self::ListOrganizations { request }
            | Self::CreateOrganization { request, .. }
            | Self::OpenOrganization { request, .. }
            | Self::PutObjects { request, .. }
            | Self::GetObject { request, .. }
            | Self::MissingObjects { request, .. }
            | Self::CreateBlock { request, .. }
            | Self::SetMetadata { request, .. }
            | Self::Publish { request, .. }
            | Self::HoldObjects { request, .. }
            | Self::ReadBlock { request, .. }
            | Self::ReadContents { request, .. }
            | Self::SetParent { request, .. }
            | Self::ListShared { request }
            | Self::ListChildren { request, .. }
            | Self::ListDeleted { request }
            | Self::ListBackrefs { request, .. }
            | Self::ListReferences { request, .. }
            | Self::ListOpenSettings { request, .. }
            | Self::Unlist { request, .. }
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
            | Self::Relay { request, .. }
            | Self::GetKeys { request, .. }
            | Self::PutKeys { request, .. }
            | Self::GetRecoveryKey { request }
            | Self::SetRecoveryKey { request, .. }
            | Self::ResealKeys { request, .. }
            | Self::Pair { request, .. }
            | Self::CreateAgent { request, .. }
            | Self::ListAgents { request, .. }
            | Self::RemoveAgent { request, .. }
            | Self::GetKeyWork { request }
            | Self::Rotate { request, .. }
            | Self::CreateEscrow { request, .. }
            | Self::PinEscrow { request, .. }
            | Self::ListMembers { request }
            | Self::SetRole { request, .. }
            | Self::RemoveMember { request, .. }
            | Self::Ping { request } => *request,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryEntry {
    pub commit: CommitId,
    pub parents: Vec<CommitId>,
    pub time: i64,
    pub pinned: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ServerMessage {
    BrowserLogin {
        request: u64,
        code: String,
        secret: String,
    },
    BrowserLoginFinished {
        approved: bool,
        message: String,
    },
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
    Organizations {
        request: u64,
        organizations: Vec<Organization>,
    },
    Invitation {
        request: u64,
        invitation: OrganizationInvitation,
    },
    OrganizationOpened {
        request: u64,
        organization: Uuid,
        role: OrganizationRole,
        kind: OrganizationKind,
        escrows: Vec<EscrowKey>,
        #[serde(with = "serde_bytes")]
        pinned: Vec<u8>,
        server: ServerAccount,
    },
    Stored {
        request: u64,
        hashes: Vec<Hash>,
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
        list: AccessList,
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
        revision: u64,
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
    Keys {
        request: u64,
        keys: KeySet,
    },
    RecoveryKey {
        request: u64,
        public: Option<[u8; 32]>,
        sealed: Vec<SealedKey>,
        own: Vec<OwnSealed>,
    },
    ContactLink {
        request: u64,
        link: ContactLink,
    },
    ContactLinks {
        request: u64,
        links: Vec<OwnContactLink>,
    },
    Contacts {
        request: u64,
        contacts: Vec<ContactRow>,
    },
    ContactsChanged,
    Paired {
        from: ClientId,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
    AgentCreated {
        request: u64,
        agent: Agent,
        token: String,
    },
    Agents {
        request: u64,
        agents: Vec<Agent>,
    },
    KeyWork {
        request: u64,
        work: KeyWork,
    },
    Members {
        request: u64,
        members: Vec<Member>,
    },
    Pong {
        request: u64,
    },
    KeyWorkChanged,
    Contents {
        request: u64,
        blocks: Vec<BlockSummary>,
        objects: Vec<ByteBuf>,
    },
}

impl ServerMessage {
    pub fn request(&self) -> Option<u64> {
        match self {
            Self::BrowserLogin { request, .. }
            | Self::Ok { request }
            | Self::Failed { request, .. }
            | Self::Authenticated { request, .. }
            | Self::Organizations { request, .. }
            | Self::Invitation { request, .. }
            | Self::OrganizationOpened { request, .. }
            | Self::Stored { request, .. }
            | Self::Contents { request, .. }
            | Self::Object { request, .. }
            | Self::Missing { request, .. }
            | Self::Block { request, .. }
            | Self::Blocks { request, .. }
            | Self::Published { request, .. }
            | Self::Rejected { request, .. }
            | Self::History { request, .. }
            | Self::Collected { request, .. }
            | Self::AccessList { request, .. }
            | Self::Session { request, .. }
            | Self::Keys { request, .. }
            | Self::RecoveryKey { request, .. }
            | Self::ContactLink { request, .. }
            | Self::ContactLinks { request, .. }
            | Self::Contacts { request, .. }
            | Self::AgentCreated { request, .. }
            | Self::Agents { request, .. }
            | Self::KeyWork { request, .. }
            | Self::Members { request, .. }
            | Self::Pong { request } => Some(*request),
            Self::HeadChanged { .. }
            | Self::BlockDeleted { .. }
            | Self::BlockChanged { .. }
            | Self::BlockRemoved { .. }
            | Self::SessionChanged { .. }
            | Self::Relayed { .. }
            | Self::Paired { .. }
            | Self::ContactsChanged
            | Self::BrowserLoginFinished { .. }
            | Self::KeyWorkChanged => None,
        }
    }
}

pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_stdvec(message)
}

pub fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}

const HELLO: &[u8] = b"be3 hello\n";

pub fn hello(build: &str) -> Vec<u8> {
    [HELLO, build.as_bytes()].concat()
}

pub fn read_hello(frame: &[u8]) -> Option<String> {
    let build = frame.strip_prefix(HELLO)?;
    String::from_utf8(build.to_vec()).ok()
}

#[cfg(test)]
mod tests;
