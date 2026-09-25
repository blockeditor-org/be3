use std::{
    collections::{HashMap, hash_map::Entry},
    path::{Path, PathBuf},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use be_commit::CommitId;
use be_graph::{Access, BlockGraph, BlockNode, BlockParent, GraphError, ObjectRefs};
use be_protocol::{
    AccessEntry, BlockSummary, ErrorCode, HistoryEntry, Workspace, WorkspaceInvitation,
    WorkspaceRole,
};
use be_store::{FileStore, Hash, ObjectStore};
use rand::TryRngCore;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{ServerError, schema};

pub const MAX_OBJECT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct Identity {
    pub account: Uuid,
    pub workspace: Uuid,
    pub role: WorkspaceRole,
}

impl Identity {
    pub fn is_administrator(&self) -> bool {
        self.role == WorkspaceRole::Administrator
    }
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub account: Uuid,
    pub email: String,
    pub display_name: String,
}

pub struct ServerStore {
    database: Mutex<Connection>,
    objects: FileStore,
    graphs: Mutex<HashMap<Uuid, BlockGraph>>,
    object_refs: std::sync::Mutex<ObjectRefs>,
}

impl ServerStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ServerError> {
        let root: PathBuf = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        let connection = Connection::open(root.join("metadata.sqlite"))?;
        schema::initialize(&connection)?;
        let object_refs = load_object_refs(&connection)?;
        Ok(Self {
            database: Mutex::new(connection),
            objects: FileStore::open(root.join("objects"))?,
            graphs: Mutex::new(HashMap::new()),
            object_refs: std::sync::Mutex::new(object_refs),
        })
    }

    pub async fn register(
        &self,
        email: &str,
        display_name: &str,
        password: &str,
    ) -> Result<(Profile, String), ServerError> {
        let email = normalize_email(email)?;
        let display_name = display_name.trim().to_owned();
        if display_name.is_empty() {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "a display name is required".into(),
            ));
        }
        if password.len() < 8 {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "a password must be at least eight characters".into(),
            ));
        }
        let hash = hash_password(password)?;
        let account = Uuid::new_v4();
        let database = self.database.lock().await;
        let existing: Option<String> = database
            .query_row(
                "SELECT id FROM accounts WHERE email = ?1",
                [&email],
                |row| row.get(0),
            )
            .optional()?;
        if existing.is_some() {
            return Err(ServerError::Refused(
                ErrorCode::EmailAlreadyRegistered,
                "that email is already registered".into(),
            ));
        }
        database.execute(
            "INSERT INTO accounts (id, email, display_name, password_hash) VALUES (?1, ?2, ?3, ?4)",
            params![account.to_string(), email, display_name, hash],
        )?;
        let token = issue_token(&database, account)?;
        Ok((
            Profile {
                account,
                email,
                display_name,
            },
            token,
        ))
    }

    pub async fn login(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(Profile, String), ServerError> {
        let email = normalize_email(email)?;
        let database = self.database.lock().await;
        let row: Option<(String, String, String)> = database
            .query_row(
                "SELECT id, display_name, password_hash FROM accounts WHERE email = ?1",
                [&email],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((id, display_name, hash)) = row else {
            return Err(ServerError::Refused(
                ErrorCode::InvalidCredentials,
                "no account matches those credentials".into(),
            ));
        };
        if !verify_password(password, &hash) {
            return Err(ServerError::Refused(
                ErrorCode::InvalidCredentials,
                "no account matches those credentials".into(),
            ));
        }
        let account = parse_uuid(&id)?;
        let token = issue_token(&database, account)?;
        Ok((
            Profile {
                account,
                email,
                display_name,
            },
            token,
        ))
    }

    pub async fn resolve_token(&self, token: &str) -> Result<Profile, ServerError> {
        let database = self.database.lock().await;
        let row: Option<(String, String, String)> = database
            .query_row(
                "SELECT accounts.id, accounts.email, accounts.display_name FROM sessions
                 JOIN accounts ON accounts.id = sessions.account_id
                 WHERE sessions.token_hash = ?1",
                [hash_token(token)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((id, email, display_name)) = row else {
            return Err(ServerError::Refused(
                ErrorCode::InvalidCredentials,
                "that session token is not valid".into(),
            ));
        };
        Ok(Profile {
            account: parse_uuid(&id)?,
            email,
            display_name,
        })
    }

    pub async fn profile(&self, account: Uuid) -> Result<Profile, ServerError> {
        let database = self.database.lock().await;
        let (email, display_name): (String, String) = database.query_row(
            "SELECT email, display_name FROM accounts WHERE id = ?1",
            [account.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(Profile {
            account,
            email,
            display_name,
        })
    }

    pub async fn logout(&self, token: &str) -> Result<(), ServerError> {
        let database = self.database.lock().await;
        database.execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            [hash_token(token)],
        )?;
        Ok(())
    }

    pub async fn invite(
        &self,
        account: Uuid,
        workspace: Uuid,
        email: &str,
        role: WorkspaceRole,
    ) -> Result<(), ServerError> {
        if self.membership(account, workspace).await? != WorkspaceRole::Administrator {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "only a workspace administrator can invite people".into(),
            ));
        }
        let email = normalize_email(email)?;
        let database = self.database.lock().await;
        database.execute(
            "DELETE FROM invitations WHERE workspace_id = ?1 AND email = ?2",
            params![workspace.to_string(), email],
        )?;
        database.execute(
            "INSERT INTO invitations (id, workspace_id, email, role, invited_by)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                workspace.to_string(),
                email,
                encode_role(role),
                account.to_string()
            ],
        )?;
        Ok(())
    }

    pub async fn list_invitations(
        &self,
        account: Uuid,
    ) -> Result<Vec<WorkspaceInvitation>, ServerError> {
        let email = self.profile(account).await?.email;
        let database = self.database.lock().await;
        let mut statement = database.prepare(
            "SELECT invitations.id, invitations.workspace_id, workspaces.name, invitations.role,
                    invitations.invited_by
             FROM invitations JOIN workspaces ON workspaces.id = invitations.workspace_id
             WHERE invitations.email = ?1
               AND NOT EXISTS (SELECT 1 FROM memberships
                               WHERE memberships.workspace_id = invitations.workspace_id
                                 AND memberships.account_id = ?2)
             ORDER BY workspaces.name",
        )?;
        let rows = statement.query_map(params![email, account.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut invitations = Vec::new();
        for row in rows {
            let (id, workspace, workspace_name, role, invited_by) = row?;
            invitations.push(WorkspaceInvitation {
                id: parse_uuid(&id)?,
                workspace: parse_uuid(&workspace)?,
                workspace_name,
                email: email.clone(),
                role: decode_role(&role)?,
                invited_by: parse_uuid(&invited_by)?,
            });
        }
        Ok(invitations)
    }

    pub async fn respond_invitation(
        &self,
        account: Uuid,
        invitation: Uuid,
        accept: bool,
    ) -> Result<(), ServerError> {
        let email = self.profile(account).await?.email;
        let database = self.database.lock().await;
        let found: Option<(String, String)> = database
            .query_row(
                "SELECT workspace_id, role FROM invitations WHERE id = ?1 AND email = ?2",
                params![invitation.to_string(), email],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((workspace, role)) = found else {
            return Err(ServerError::Refused(
                ErrorCode::InvitationNotFound,
                "that invitation does not exist".into(),
            ));
        };
        if accept {
            database.execute(
                "INSERT OR IGNORE INTO memberships (workspace_id, account_id, role)
                 VALUES (?1, ?2, ?3)",
                params![workspace, account.to_string(), role],
            )?;
        }
        database.execute(
            "DELETE FROM invitations WHERE id = ?1",
            [invitation.to_string()],
        )?;
        Ok(())
    }

    pub async fn list_workspaces(&self, account: Uuid) -> Result<Vec<Workspace>, ServerError> {
        let database = self.database.lock().await;
        let mut statement = database.prepare(
            "SELECT workspaces.id, workspaces.name, memberships.role FROM memberships
             JOIN workspaces ON workspaces.id = memberships.workspace_id
             WHERE memberships.account_id = ?1
             ORDER BY workspaces.name",
        )?;
        let rows = statement.query_map([account.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut workspaces = Vec::new();
        for row in rows {
            let (id, name, role) = row?;
            workspaces.push(Workspace {
                id: parse_uuid(&id)?,
                name,
                role: decode_role(&role)?,
            });
        }
        Ok(workspaces)
    }

    pub async fn create_workspace(
        &self,
        account: Uuid,
        name: &str,
    ) -> Result<Workspace, ServerError> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "a workspace name is required".into(),
            ));
        }
        let id = Uuid::new_v4();
        let database = self.database.lock().await;
        database.execute(
            "INSERT INTO workspaces (id, name, owner_id) VALUES (?1, ?2, ?3)",
            params![id.to_string(), name, account.to_string()],
        )?;
        database.execute(
            "INSERT INTO memberships (workspace_id, account_id, role) VALUES (?1, ?2, 'administrator')",
            params![id.to_string(), account.to_string()],
        )?;
        Ok(Workspace {
            id,
            name,
            role: WorkspaceRole::Administrator,
        })
    }

    pub async fn issue_session(&self, account: Uuid) -> Result<String, ServerError> {
        let database = self.database.lock().await;
        issue_token(&database, account)
    }

    pub async fn membership(
        &self,
        account: Uuid,
        workspace: Uuid,
    ) -> Result<WorkspaceRole, ServerError> {
        let database = self.database.lock().await;
        let role: Option<String> = database
            .query_row(
                "SELECT role FROM memberships WHERE workspace_id = ?1 AND account_id = ?2",
                params![workspace.to_string(), account.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(role) = role else {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "that account is not a member of this workspace".into(),
            ));
        };
        decode_role(&role)
    }

    pub async fn add_member(
        &self,
        workspace: Uuid,
        account: Uuid,
        role: WorkspaceRole,
    ) -> Result<(), ServerError> {
        let database = self.database.lock().await;
        database.execute(
            "INSERT OR REPLACE INTO memberships (workspace_id, account_id, role)
             VALUES (?1, ?2, ?3)",
            params![
                workspace.to_string(),
                account.to_string(),
                encode_role(role)
            ],
        )?;
        Ok(())
    }

    pub fn put_object(&self, bytes: &[u8]) -> Result<Hash, ServerError> {
        if bytes.len() > MAX_OBJECT_BYTES {
            return Err(ServerError::Refused(
                ErrorCode::ObjectTooLarge,
                format!("an object may not exceed {MAX_OBJECT_BYTES} bytes"),
            ));
        }
        Ok(self.objects.put(bytes)?)
    }

    pub fn get_object(&self, hash: Hash) -> Result<Option<Vec<u8>>, ServerError> {
        Ok(self.objects.get(hash)?)
    }

    pub fn get_object_range(
        &self,
        hash: Hash,
        offset: u64,
        length: u64,
    ) -> Result<Option<Vec<u8>>, ServerError> {
        if !self.objects.has(hash)? {
            return Ok(None);
        }
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let length = usize::try_from(length).unwrap_or(usize::MAX);
        Ok(Some(self.objects.get_range(hash, offset, length)?))
    }

    pub fn missing_objects(&self, hashes: &[Hash]) -> Result<Vec<Hash>, ServerError> {
        let mut missing = Vec::new();
        for hash in hashes {
            if !self.objects.has(*hash)? {
                missing.push(*hash);
            }
        }
        Ok(missing)
    }
}

fn load_object_refs(connection: &Connection) -> Result<ObjectRefs, ServerError> {
    let mut statement = connection.prepare("SELECT hash, count FROM object_refs")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut refs = ObjectRefs::new();
    for row in rows {
        let (hash, count) = row?;
        let hash = Hash::from_hex(&hash).ok_or(ServerError::Corrupt)?;
        for _ in 0..count.max(0) {
            refs.retain(&[hash]);
        }
    }
    Ok(refs)
}

pub fn normalize_email(email: &str) -> Result<String, ServerError> {
    let email = email.trim().to_ascii_lowercase();
    if !email.contains('@') || email.len() < 3 {
        return Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            "that email address is not valid".into(),
        ));
    }
    Ok(email)
}

fn hash_password(password: &str) -> Result<String, ServerError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| ServerError::Corrupt)
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

fn issue_token(database: &Connection, account: Uuid) -> Result<String, ServerError> {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("the operating system has entropy");
    let token = Hash::from_bytes(bytes).to_hex();
    database.execute(
        "INSERT INTO sessions (token_hash, account_id) VALUES (?1, ?2)",
        params![hash_token(&token), account.to_string()],
    )?;
    Ok(token)
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    Hash::from_bytes(hasher.finalize().into()).to_hex()
}

pub fn parse_uuid(value: &str) -> Result<Uuid, ServerError> {
    Uuid::parse_str(value).map_err(|_| ServerError::Corrupt)
}

pub fn decode_role(value: &str) -> Result<WorkspaceRole, ServerError> {
    match value {
        "administrator" => Ok(WorkspaceRole::Administrator),
        "editor" => Ok(WorkspaceRole::Editor),
        _ => Err(ServerError::Corrupt),
    }
}

pub fn encode_role(role: WorkspaceRole) -> &'static str {
    match role {
        WorkspaceRole::Administrator => "administrator",
        WorkspaceRole::Editor => "editor",
    }
}

pub fn encode_access(access: Access) -> Option<&'static str> {
    match access {
        Access::None => None,
        Access::KnowExists => Some("know_exists"),
        Access::View => Some("view"),
        Access::Edit => Some("edit"),
    }
}

pub fn decode_access(value: &str) -> Result<Access, ServerError> {
    match value {
        "know_exists" => Ok(Access::KnowExists),
        "view" => Ok(Access::View),
        "edit" => Ok(Access::Edit),
        _ => Err(ServerError::Corrupt),
    }
}

pub fn encode_parent(parent: BlockParent) -> (i64, Option<String>) {
    match parent {
        BlockParent::Detached => (0, None),
        BlockParent::Root => (1, None),
        BlockParent::Block(id) => (2, Some(id.to_string())),
    }
}

pub fn decode_parent(kind: i64, id: Option<String>) -> Result<BlockParent, ServerError> {
    match (kind, id) {
        (0, _) => Ok(BlockParent::Detached),
        (1, _) => Ok(BlockParent::Root),
        (2, Some(id)) => Ok(BlockParent::Block(parse_uuid(&id)?)),
        _ => Err(ServerError::Corrupt),
    }
}

impl From<GraphError> for ServerError {
    fn from(error: GraphError) -> Self {
        match error {
            GraphError::NotFound(id) => Self::Refused(
                ErrorCode::BlockNotFound,
                format!("block {id} does not exist"),
            ),
            GraphError::AlreadyExists(id) => Self::Refused(
                ErrorCode::BlockAlreadyExists,
                format!("block {id} already exists"),
            ),
            GraphError::ParentCycle => Self::Refused(
                ErrorCode::ParentCycle,
                "that parent would make the block its own ancestor".into(),
            ),
        }
    }
}

pub struct BlockState {
    pub summary: BlockSummary,
    pub history: Vec<HistoryEntry>,
}

pub struct Collected {
    pub blocks: Vec<Uuid>,
    pub objects: usize,
}

pub struct AccessList {
    pub entries: Vec<AccessEntry>,
}

impl ServerStore {
    pub(crate) async fn with_graph<R>(
        &self,
        workspace: Uuid,
        action: impl FnOnce(&mut BlockGraph, &Connection) -> Result<R, ServerError>,
    ) -> Result<R, ServerError> {
        let database = self.database.lock().await;
        let mut graphs = self.graphs.lock().await;
        let graph = match graphs.entry(workspace) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(load_graph(&database, workspace)?),
        };
        let outcome = action(graph, &database);
        if outcome.is_err() {
            graphs.remove(&workspace);
        }
        outcome
    }

    pub(crate) fn object_refs(&self) -> std::sync::MutexGuard<'_, ObjectRefs> {
        self.object_refs
            .lock()
            .expect("object refs are never poisoned")
    }

    pub(crate) fn objects(&self) -> &FileStore {
        &self.objects
    }
}

pub(crate) fn load_graph(
    connection: &Connection,
    workspace: Uuid,
) -> Result<BlockGraph, ServerError> {
    let mut graph = BlockGraph::new();
    let mut nodes = Vec::new();
    {
        let mut statement = connection.prepare(
            "SELECT id, content_type, author, parent_kind, parent_id, head, metadata, version FROM blocks
             WHERE workspace_id = ?1",
        )?;
        let rows = statement.query_map([workspace.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Vec<u8>>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?;
        for row in rows {
            nodes.push(row?);
        }
    }
    for (id, content_type, author, parent_kind, parent_id, head, metadata, version) in &nodes {
        let mut node = BlockNode::new(
            parse_uuid(content_type)?,
            parse_uuid(author)?,
            BlockParent::Detached,
        );
        node.head = head
            .as_deref()
            .map(|head| {
                Hash::from_hex(head)
                    .map(CommitId::from_hash)
                    .ok_or(ServerError::Corrupt)
            })
            .transpose()?;
        node.metadata.clone_from(metadata);
        node.version = u64::try_from(*version).map_err(|_| ServerError::Corrupt)?;
        let _ = (parent_kind, parent_id);
        graph.insert(parse_uuid(id)?, node)?;
    }
    for (id, _, _, parent_kind, parent_id, _, _, _) in &nodes {
        let parent = decode_parent(*parent_kind, parent_id.clone())?;
        graph.set_parent(parse_uuid(id)?, parent)?;
    }
    {
        let mut statement = connection
            .prepare("SELECT block_id, reference_id FROM block_edges WHERE workspace_id = ?1")?;
        let rows = statement.query_map([workspace.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (block, reference) = row?;
            graph.apply_references(parse_uuid(&block)?, &[parse_uuid(&reference)?], &[])?;
        }
    }
    {
        let mut statement = connection.prepare(
            "SELECT block_id, account_id, access FROM block_access WHERE workspace_id = ?1",
        )?;
        let rows = statement.query_map([workspace.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (block, account, access) = row?;
            graph.grant(
                parse_uuid(&block)?,
                parse_uuid(&account)?,
                decode_access(&access)?,
            )?;
        }
    }
    Ok(graph)
}
