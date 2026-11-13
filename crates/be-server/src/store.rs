pub mod invite_codes;
pub(crate) mod passwords;
pub(crate) mod portal;

/// A test's way to keep a store job running for as long as it likes.
#[cfg(test)]
pub(crate) mod held {
    use std::{
        collections::HashMap,
        sync::{Mutex, mpsc},
    };

    use uuid::Uuid;

    type Held = (tokio::sync::oneshot::Sender<()>, mpsc::Receiver<()>);

    static HELD: Mutex<Option<HashMap<Uuid, Held>>> = Mutex::new(None);

    /// Makes the next graph job for `organization` tell `reached` when it starts, then wait on
    /// its blocking thread until the returned sender sends or is dropped.
    pub(crate) fn hold(
        organization: Uuid,
    ) -> (tokio::sync::oneshot::Receiver<()>, mpsc::Sender<()>) {
        let (reached, started) = tokio::sync::oneshot::channel();
        let (release, held) = mpsc::channel();
        HELD.lock()
            .unwrap()
            .get_or_insert_with(HashMap::new)
            .insert(organization, (reached, held));
        (started, release)
    }

    pub(super) fn wait(organization: Uuid) {
        let held = HELD
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|held| held.remove(&organization));
        if let Some((reached, held)) = held {
            let _ = reached.send(());
            let _ = held.recv();
        }
    }
}
pub(crate) mod usage;
pub use portal::{AccountSession, MailLink};
use std::{
    collections::{BTreeMap, HashMap, hash_map::Entry},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
pub use usage::{DEFAULT_ACCOUNT_QUOTA, DEFAULT_UPLOAD_GRACE, StorageLimits};

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use be_block_commit::CommitId;
use be_block_graph::{Access, BlockGraph, BlockNode, BlockParent, GraphError, ObjectRefs};
use be_block_protocol::{
    Agent, ErrorCode, Organization, OrganizationInvitation, OrganizationKind, OrganizationRole,
};
use be_block_store::Hash;
use rand::TryRngCore;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{ServerError, schema};

pub use be_block_protocol::MAX_OBJECT_BYTES;

#[derive(Clone, Copy, Debug)]
pub struct Identity {
    pub account: Uuid,
    pub organization: Uuid,
    pub role: OrganizationRole,
    /// For a block's agent, the block it is bound to: it reaches that block and the blocks
    /// under it and nothing else (`blocks::Visibility`).
    pub bound: Option<Uuid>,
}

impl Identity {
    pub fn is_administrator(&self) -> bool {
        self.role == OrganizationRole::Administrator
    }
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub account: Uuid,
    pub email: String,
    pub display_name: String,
}

/// The server's metadata and objects. Cloning it is cheap and shares everything. Its SQL runs
/// as owned jobs on tokio's blocking pool (`with_database`, `with_graph`), one at a time under
/// the database lock, so a slow call never stalls the runtime's threads and with them every
/// other connection.
#[derive(Clone)]
pub struct ServerStore(Arc<Shared>);

impl std::ops::Deref for ServerStore {
    type Target = Shared;

    fn deref(&self) -> &Shared {
        &self.0
    }
}

/// Revisions handed out between two raises of `revision_floor`: the floor is written once per
/// this many changes, not once per change.
const REVISIONS_RESERVED: u64 = 1 << 16;

/// Begins a transaction that writes, taking the write lock up front. Another connection to the
/// database (a test or tool opening the store beside a running server) may hold it; a deferred
/// transaction that read first and then wrote would fail at once with `SQLITE_BUSY`, since
/// SQLite never waits on a read upgrading to a write, where this one waits out the busy timeout.
pub(crate) fn write_transaction(
    connection: &Connection,
) -> rusqlite::Result<rusqlite::Transaction<'_>> {
    rusqlite::Transaction::new_unchecked(connection, rusqlite::TransactionBehavior::Immediate)
}

/// Raises the stored floor past every revision this run may hand out before it raises it again,
/// and answers where this run counts from: `from`, or at the start the floor a previous run left.
/// A summary's revision then only grows across restarts, which devices rely on to tell newer
/// from older (be-block-client's refs keep the newest summary of each block).
fn reserve_revisions(connection: &Connection, from: Option<u64>) -> Result<u64, ServerError> {
    let start = match from {
        Some(from) => from,
        None => connection
            .query_row(
                "SELECT revision FROM revision_floor WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map_or(0, |floor| u64::try_from(floor).unwrap_or_default()),
    };
    let floor = i64::try_from(start + REVISIONS_RESERVED).unwrap_or(i64::MAX);
    connection.execute(
        "INSERT INTO revision_floor (id, revision) VALUES (1, ?1)
         ON CONFLICT (id) DO UPDATE SET revision = MAX(revision, excluded.revision)",
        params![floor],
    )?;
    Ok(start)
}

pub struct Shared {
    database: Arc<Mutex<Connection>>,
    pub(crate) session_changes: tokio::sync::watch::Sender<u64>,
    objects: crate::objects::Objects,
    graphs: std::sync::Mutex<HashMap<Uuid, Arc<std::sync::RwLock<Cached>>>>,
    /// A read-only connection beside the writer (the database is in WAL mode), for reads that
    /// must not wait behind writes: whether a reader may download an object.
    reader: std::sync::Mutex<Connection>,
    revision: std::sync::atomic::AtomicU64,
    /// How far `revision` may run before `revision_floor` is raised again (`REVISIONS_RESERVED`).
    reserved: std::sync::atomic::AtomicU64,
    graph_loads: std::sync::atomic::AtomicU64,
    object_refs: std::sync::Mutex<ObjectRefs>,
    pub(crate) passwords: passwords::Passwords,
    server: crate::organization::ServerKey,
    policies: std::sync::Mutex<HashMap<Uuid, crate::organization::HeldPolicies>>,
    limits: StorageLimits,
    /// Each payer whose `account_usage` row changes, for the connections following it.
    pub(crate) usage_changes: tokio::sync::broadcast::Sender<Uuid>,
}

/// Agents one account may have created and not yet removed, not counting blocks' agents.
pub const MAX_AGENTS_PER_CREATOR: i64 = 16;

/// Agents bound to one block (`create_agent`'s `bound_to`).
pub const MAX_AGENTS_PER_BLOCK: i64 = 4;

/// Blocks' agents one account may have made and not yet removed, over all blocks.
pub const MAX_BLOCK_AGENTS_PER_CREATOR: i64 = 64;

/// Payers whose usage changed that a connection following its storage may fall behind by;
/// one further behind reads its usage again rather than missing a change.
pub const USAGE_CHANGES_QUEUED: usize = 256;

impl ServerStore {
    /// Opens the server in `root` with the default limits and its objects in `root/objects`.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ServerError> {
        Self::open_with(root, StorageLimits::DEFAULT, None)
    }

    /// Opens the server in `root`, with its objects in `bucket` (behind a cache in
    /// `root/object-cache`) or, with none, in `root/objects`.
    pub fn open_with(
        root: impl AsRef<Path>,
        limits: StorageLimits,
        bucket: Option<&crate::objects::BucketConfig>,
    ) -> Result<Self, ServerError> {
        let root: PathBuf = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        let connection = Connection::open(root.join("metadata.sqlite"))?;
        connection.busy_timeout(Duration::from_secs(5))?;
        schema::initialize(&connection)?;
        let object_refs = load_object_refs(&connection)?;
        let server = crate::organization::ServerKey::load_or_create(&connection)?;
        let objects = crate::objects::Objects::open(&root, bucket)?;
        let reader = Connection::open_with_flags(
            root.join("metadata.sqlite"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        reader.busy_timeout(Duration::from_secs(5))?;
        let floor = reserve_revisions(&connection, None)?;
        Ok(Self(Arc::new(Shared {
            database: Arc::new(Mutex::new(connection)),
            session_changes: tokio::sync::watch::channel(0).0,
            objects,
            graphs: std::sync::Mutex::new(HashMap::new()),
            reader: std::sync::Mutex::new(reader),
            revision: std::sync::atomic::AtomicU64::new(floor),
            reserved: std::sync::atomic::AtomicU64::new(floor + REVISIONS_RESERVED),
            graph_loads: std::sync::atomic::AtomicU64::new(0),
            object_refs: std::sync::Mutex::new(object_refs),
            passwords: passwords::Passwords::default(),
            server,
            policies: std::sync::Mutex::new(HashMap::new()),
            limits,
            usage_changes: tokio::sync::broadcast::channel(USAGE_CHANGES_QUEUED).0,
        })))
    }

    pub async fn create_local_profile(
        &self,
        display_name: &str,
    ) -> Result<(Profile, String), ServerError> {
        let display_name = crate::limits::name("a display name", display_name)?.to_owned();
        let account = Uuid::new_v4();
        self.with_database(move |_, database| {
            let transaction = write_transaction(database)?;
            transaction.execute(
                "INSERT INTO accounts (id, email, display_name, password_hash, local_profile) VALUES (?1, ?2, ?3, '', 1)",
                params![
                    account.to_string(),
                    format!("{account}@local.invalid"),
                    display_name
                ],
            )?;
            let personal = personal_organization(&transaction)?;
            transaction.execute(
                "INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, 'editor')",
                params![personal.to_string(), account.to_string()],
            )?;
            let token = issue_token(&transaction, account)?;
            transaction.commit()?;
            Ok((
                Profile {
                    account,
                    email: String::new(),
                    display_name: display_name.to_owned(),
                },
                token,
            ))
        })
        .await
    }

    pub async fn register(
        &self,
        email: &str,
        display_name: &str,
        password: &str,
    ) -> Result<(Profile, String), ServerError> {
        let email = normalize_email(email)?;
        let display_name = crate::limits::name("a display name", display_name)?.to_owned();
        if password.len() < 8 {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "a password must be at least eight characters".into(),
            ));
        }
        let hash = self.hash(password.to_owned(), "").await?;
        let account = Uuid::new_v4();
        self.with_database(move |_, database| {
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
            let transaction = write_transaction(database)?;
            transaction.execute(
                "INSERT INTO accounts (id, email, display_name, password_hash) VALUES (?1, ?2, ?3, ?4)",
                params![account.to_string(), email, display_name, hash],
            )?;
            let personal = personal_organization(&transaction)?;
            transaction.execute(
                "INSERT OR IGNORE INTO memberships (organization_id, account_id, role)
                 VALUES (?1, ?2, 'editor')",
                params![personal.to_string(), account.to_string()],
            )?;
            let token = issue_token(&transaction, account)?;
            transaction.commit()?;
            Ok((
                Profile {
                    account,
                    email,
                    display_name,
                },
                token,
            ))
        })
        .await
    }

    pub async fn login_browser(
        &self,
        email: &str,
        password: &str,
        address: &str,
    ) -> Result<(Profile, String), ServerError> {
        let email = normalize_email(email)?;
        if let Some(wait) = self
            .passwords
            .locked_out(&email, address, false, Instant::now())
        {
            return Err(ServerError::Refused(
                ErrorCode::InvalidCredentials,
                format!(
                    "too many failed sign-ins; try again in {} seconds",
                    wait.as_secs().max(1)
                ),
            ));
        }
        let row: Option<(String, String, String)> = {
            let email = email.clone();
            self.with_database(move |_, database| {
                Ok(database
                    .query_row(
                        "SELECT id, display_name, password_hash FROM accounts WHERE email = ?1",
                        [&email],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()?)
            })
            .await?
        };
        let hash = row
            .as_ref()
            .map_or_else(|| dummy_hash().to_owned(), |(_, _, hash)| hash.clone());
        let matches = self.verify(password.to_owned(), hash, address).await?;
        let Some((id, display_name, previous_hash)) = row.filter(|_| matches) else {
            self.passwords.failed(&email, address, Instant::now());
            return Err(ServerError::Refused(
                ErrorCode::InvalidCredentials,
                "no account matches those credentials".into(),
            ));
        };
        self.passwords.succeeded(&email, address, Instant::now());
        let account = parse_uuid(&id)?;
        self.with_database(move |_, database| {
            let transaction = write_transaction(database)?;
            let unchanged: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1 AND password_hash = ?2)",
                params![id, previous_hash],
                |row| row.get(0),
            )?;
            if !unchanged {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidCredentials,
                    "Your credentials changed. Sign in again.".into(),
                ));
            }
            let token = issue_token(&transaction, account)?;
            transaction.execute(
                "UPDATE sessions SET kind = 'browser', expires = ?2 WHERE token_hash = ?1",
                params![hash_token(&token), portal::now() + 30 * 86400],
            )?;
            transaction.commit()?;
            Ok((
                Profile {
                    account,
                    email,
                    display_name,
                },
                token,
            ))
        })
        .await
    }

    async fn hash(&self, password: String, address: &str) -> Result<String, ServerError> {
        self.passwords
            .run(address, move || hash_password(&password))
            .await?
    }

    async fn verify(
        &self,
        password: String,
        hash: String,
        address: &str,
    ) -> Result<bool, ServerError> {
        self.passwords
            .run(address, move || verify_password(&password, &hash))
            .await
    }

    pub async fn checkpoint(&self) -> Result<(), ServerError> {
        self.with_database(|_, database| {
            database.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            Ok(())
        })
        .await
    }

    pub async fn resolve_token(&self, token: &str) -> Result<Profile, ServerError> {
        let token = token.to_owned();
        self.with_database(move |_, database| {
            let row: Option<(String, String, String)> = database
                .query_row(
                    "SELECT accounts.id, CASE WHEN accounts.local_profile THEN '' ELSE accounts.email END, accounts.display_name FROM sessions
                     JOIN accounts ON accounts.id = sessions.account_id
                     WHERE sessions.token_hash = ?1 AND sessions.kind = 'app'",
                    [hash_token(&token)],
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
        })
        .await
    }

    pub async fn profile(&self, account: Uuid) -> Result<Profile, ServerError> {
        self.with_database(move |_, database| {
            let (email, display_name): (String, String) = database.query_row(
                "SELECT CASE WHEN local_profile THEN '' ELSE email END, display_name FROM accounts WHERE id = ?1",
                [account.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            Ok(Profile {
                account,
                email,
                display_name,
            })
        })
        .await
    }

    pub async fn logout(&self, token: &str) -> Result<(), ServerError> {
        let token = token.to_owned();
        self.with_database(move |store, database| {
            database.execute(
                "DELETE FROM sessions WHERE token_hash = ?1",
                [hash_token(&token)],
            )?;
            store.sessions_changed();
            Ok(())
        })
        .await
    }

    pub async fn invite(
        &self,
        account: Uuid,
        invitation: Uuid,
        organization: Uuid,
        email: &str,
        role: OrganizationRole,
        sealed: Vec<u8>,
        code: Vec<u8>,
        escrow: Vec<u8>,
    ) -> Result<(), ServerError> {
        if self.membership(account, organization).await? != OrganizationRole::Administrator {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "only an organization administrator can invite people".into(),
            ));
        }
        let email = normalize_email(email)?;
        for (what, bytes) in [
            ("an invitation", &sealed),
            ("an invitation code", &code),
            ("an invitation's escrow", &escrow),
        ] {
            crate::limits::at_most(what, bytes.len(), crate::limits::MAX_SEALED_BYTES)?;
        }
        self.with_database(move |_, database| {
            let linked: bool = database.query_row(
                "SELECT EXISTS(SELECT 1 FROM contact_links WHERE id = ?1)",
                [invitation.to_string()],
                |row| row.get(0),
            )?;
            if linked {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "an invitation needs an id of its own".into(),
                ));
            }
            let outstanding: i64 = database
                .prepare_cached(
                    "SELECT COUNT(*) FROM invitations WHERE organization_id = ?1 AND email != ?2",
                )?
                .query_row(params![organization.to_string(), email], |row| row.get(0))?;
            if outstanding >= crate::limits::MAX_INVITATIONS_PER_ORGANIZATION {
                return Err(crate::limits::too_many(
                    "invitations an organization has outstanding",
                    crate::limits::MAX_INVITATIONS_PER_ORGANIZATION,
                ));
            }
            database.execute(
                "DELETE FROM invitations WHERE organization_id = ?1 AND email = ?2",
                params![organization.to_string(), email],
            )?;
            let inserted = database.execute(
                "INSERT OR IGNORE INTO invitations
                    (id, organization_id, email, role, invited_by, sealed, code, escrow)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    invitation.to_string(),
                    organization.to_string(),
                    email,
                    encode_role(role),
                    account.to_string(),
                    sealed,
                    code,
                    escrow
                ],
            )?;
            if inserted == 0 {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "an invitation with that id already exists".into(),
                ));
            }
            Ok(())
        })
        .await
    }

    pub async fn invitation(
        &self,
        account: Uuid,
        invitation: Uuid,
    ) -> Result<OrganizationInvitation, ServerError> {
        let email = self.profile(account).await?.email;
        self.with_database(move |_, database| {
            let found = database
                .query_row(
                    "SELECT invitations.organization_id, organizations.name, invitations.role,
                            invitations.invited_by, invitations.sealed, invitations.escrow
                     FROM invitations JOIN organizations ON organizations.id = invitations.organization_id
                     WHERE invitations.id = ?1 AND invitations.email = ?2",
                    params![invitation.to_string(), email],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, Vec<u8>>(4)?,
                            row.get::<_, Vec<u8>>(5)?,
                        ))
                    },
                )
                .optional()?;
            let Some((organization, organization_name, role, invited_by, sealed, escrow)) = found
            else {
                return Err(ServerError::Refused(
                    ErrorCode::InvitationNotFound,
                    "that invitation does not exist, or was sent to another email address".into(),
                ));
            };
            let organization = parse_uuid(&organization)?;
            if is_member(&database, account, organization)? {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "you are already a member of that organization".into(),
                ));
            }
            Ok(OrganizationInvitation {
                id: invitation,
                organization,
                organization_name,
                email,
                role: decode_role(&role)?,
                invited_by: parse_uuid(&invited_by)?,
                sealed,
                escrow,
            })
        })
        .await
    }

    pub async fn accept_invitation(
        &self,
        account: Uuid,
        invitation: Uuid,
        card: Vec<u8>,
        contact: Vec<u8>,
        escrow: Vec<u8>,
    ) -> Result<Uuid, ServerError> {
        for (what, bytes) in [("a contact card", &card), ("a sealed contact", &contact)] {
            crate::limits::at_most(what, bytes.len(), crate::limits::MAX_SEALED_BYTES)?;
        }
        let found = self.invitation(account, invitation).await?;
        self.with_database(move |_, database| {
            let transaction = write_transaction(database)?;
            let code: Vec<u8> = transaction.query_row(
                "SELECT code FROM invitations WHERE id = ?1",
                [invitation.to_string()],
                |row| row.get(0),
            )?;
            let linked = transaction.execute(
                "INSERT OR IGNORE INTO contact_links (id, owner_id, card, code, accepter_id, accepter_card)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    invitation.to_string(),
                    found.invited_by.to_string(),
                    found.sealed,
                    code,
                    account.to_string(),
                    card
                ],
            )?;
            if linked == 0 {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "this invitation's id is already another contact link's".into(),
                ));
            }
            crate::contacts::befriend(&transaction, found.invited_by, &[], account, &contact)?;
            crate::keys::pin_escrow(&transaction, account, found.organization, &escrow)?;
            transaction.execute(
                "INSERT OR IGNORE INTO memberships (organization_id, account_id, role)
                 VALUES (?1, ?2, ?3)",
                params![
                    found.organization.to_string(),
                    account.to_string(),
                    encode_role(found.role)
                ],
            )?;
            let removed = transaction.execute(
                "DELETE FROM invitations WHERE id = ?1",
                [invitation.to_string()],
            )?;
            if removed == 0 {
                return Err(ServerError::Refused(
                    ErrorCode::InvitationNotFound,
                    "that invitation was just used".into(),
                ));
            }
            transaction.commit()?;
            Ok(found.invited_by)
        })
        .await
    }

    pub async fn list_organizations(
        &self,
        account: Uuid,
    ) -> Result<Vec<Organization>, ServerError> {
        self.with_database(move |_, database| {
            let mut statement = database.prepare(
                "SELECT organizations.id, organizations.name, organizations.kind, memberships.role
                 FROM memberships
                 JOIN organizations ON organizations.id = memberships.organization_id
                 WHERE memberships.account_id = ?1
                 ORDER BY organizations.kind = 'company', organizations.name",
            )?;
            let rows = statement.query_map([account.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            let mut organizations = Vec::new();
            for row in rows {
                let (id, name, kind, role) = row?;
                organizations.push(Organization {
                    id: parse_uuid(&id)?,
                    name,
                    kind: decode_kind(&kind)?,
                    role: decode_role(&role)?,
                });
            }
            Ok(organizations)
        })
        .await
    }

    pub async fn create_organization(
        &self,
        account: Uuid,
        name: &str,
    ) -> Result<Organization, ServerError> {
        let name = crate::limits::name("an organization name", name)?.to_owned();
        let id = Uuid::new_v4();
        self.with_database(move |_, database| {
            let created: i64 = database
                .prepare_cached("SELECT COUNT(*) FROM organizations WHERE owner_id = ?1")?
                .query_row([account.to_string()], |row| row.get(0))?;
            if created >= crate::limits::MAX_ORGANIZATIONS_PER_ACCOUNT {
                return Err(crate::limits::too_many(
                    "organizations one account creates",
                    crate::limits::MAX_ORGANIZATIONS_PER_ACCOUNT,
                ));
            }
            database.execute(
                "INSERT INTO organizations (id, name, owner_id) VALUES (?1, ?2, ?3)",
                params![id.to_string(), name, account.to_string()],
            )?;
            database.execute(
                "INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, 'administrator')",
                params![id.to_string(), account.to_string()],
            )?;
            Ok(Organization {
                id,
                name,
                kind: OrganizationKind::Company,
                role: OrganizationRole::Administrator,
            })
        })
        .await
    }

    pub async fn personal_organization(&self) -> Result<Uuid, ServerError> {
        self.with_database(move |_, database| personal_organization(&database))
            .await
    }

    pub async fn join_personal_organization(&self, account: Uuid) -> Result<Uuid, ServerError> {
        self.with_database(move |_, database| {
            let organization = personal_organization(&database)?;
            database.execute(
                "INSERT OR IGNORE INTO memberships (organization_id, account_id, role)
                 VALUES (?1, ?2, 'editor')",
                params![organization.to_string(), account.to_string()],
            )?;
            Ok(organization)
        })
        .await
    }

    /// Makes an agent of `caller`'s in `organization`. One `bound_to` a block, which the caller
    /// must manage, is granted Edit on it at once and is counted against the block's
    /// `MAX_AGENTS_PER_BLOCK` rather than the caller's `MAX_AGENTS_PER_CREATOR`; it also answers
    /// the block's audience before the grant, to announce the change to.
    pub async fn create_agent(
        &self,
        caller: Uuid,
        organization: Uuid,
        name: &str,
        recovery: [u8; 32],
        signing: [u8; 32],
        contact: Vec<u8>,
        escrow: Vec<u8>,
        bound_to: Option<Uuid>,
    ) -> Result<(Agent, String, std::collections::BTreeSet<Uuid>), ServerError> {
        let role = self.membership(caller, organization).await?;
        let name = crate::limits::name("an agent's name", name)?.to_owned();
        crate::limits::at_most(
            "a contact card",
            contact.len(),
            crate::limits::MAX_SEALED_BYTES,
        )?;
        crate::limits::at_most(
            "a pinned escrow",
            escrow.len(),
            crate::limits::MAX_SEALED_BYTES,
        )?;
        let account = Uuid::new_v4();
        let email = format!("agent-{}@agents.invalid", account.simple());
        self.with_graph(organization, move |_, graph, database| {
            let mut before = std::collections::BTreeSet::new();
            if let Some(block) = bound_to {
                let seen = crate::blocks::visibility(Identity {
                    account: caller,
                    organization,
                    role,
                    bound: None,
                });
                if !graph.is_live(block) || !seen.can_manage(graph, block) {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("block {block} is not managed by this account"),
                    ));
                }
                if graph.owner_only_root(block).is_some() {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("block {block} is readable by its author alone"),
                    ));
                }
                before = graph.audience(block);
            }
            let transaction = write_transaction(database)?;
            let (created, most, what): (i64, i64, &str) = match bound_to {
                Some(block) => (
                    transaction.query_row(
                        "SELECT count(*) FROM agents WHERE organization_id = ?1 AND bound_to = ?2",
                        params![organization.to_string(), block.to_string()],
                        |row| row.get(0),
                    )?,
                    MAX_AGENTS_PER_BLOCK,
                    "a block",
                ),
                None => (
                    transaction.query_row(
                        "SELECT count(*) FROM agents WHERE created_by = ?1 AND bound_to IS NULL",
                        [caller.to_string()],
                        |row| row.get(0),
                    )?,
                    MAX_AGENTS_PER_CREATOR,
                    "an account",
                ),
            };
            if created >= most {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    format!("{what} may have at most {most} agents; remove one first"),
                ));
            }
            let made: i64 = transaction.query_row(
                "SELECT count(*) FROM agents WHERE created_by = ?1 AND bound_to IS NOT NULL",
                [caller.to_string()],
                |row| row.get(0),
            )?;
            if bound_to.is_some() && made >= MAX_BLOCK_AGENTS_PER_CREATOR {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    format!(
                        "an account may have at most {MAX_BLOCK_AGENTS_PER_CREATOR} blocks' agents; remove one first"
                    ),
                ));
            }
            transaction.execute(
                "INSERT INTO accounts (id, email, display_name, password_hash) VALUES (?1, ?2, ?3, '!')",
                params![account.to_string(), email, name],
            )?;
            transaction.execute(
                "INSERT INTO agents (account_id, organization_id, created_by, bound_to)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    account.to_string(),
                    organization.to_string(),
                    caller.to_string(),
                    bound_to.map(|block| block.to_string())
                ],
            )?;
            transaction.execute(
                "INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, ?3)",
                params![
                    organization.to_string(),
                    account.to_string(),
                    encode_role(OrganizationRole::Editor)
                ],
            )?;
            transaction.execute(
                "INSERT INTO recovery_keys (account_id, public, signing) VALUES (?1, ?2, ?3)",
                params![account.to_string(), recovery.to_vec(), signing.to_vec()],
            )?;
            crate::contacts::befriend(&transaction, caller, &[], account, &contact)?;
            crate::keys::pin_escrow(&transaction, account, organization, &escrow)?;
            if let Some(block) = bound_to {
                crate::blocks::revoke_or_grant(
                    graph,
                    &transaction,
                    organization,
                    block,
                    account,
                    Access::Edit,
                )?;
            }
            let token = issue_token(&transaction, account)?;
            transaction.commit()?;
            Ok((
                Agent {
                    account,
                    name,
                    email,
                },
                token,
                before,
            ))
        })
        .await
    }

    pub async fn list_agents(
        &self,
        caller: Uuid,
        organization: Uuid,
    ) -> Result<Vec<Agent>, ServerError> {
        self.with_database(move |_, database| {
            if !is_member(&database, caller, organization)? {
                return Err(not_a_member());
            }
            let mut statement = database.prepare(
                "SELECT accounts.id, accounts.display_name, accounts.email FROM agents
                 JOIN accounts ON accounts.id = agents.account_id
                 WHERE agents.organization_id = ?1 AND agents.created_by = ?2
                    AND agents.bound_to IS NULL
                 ORDER BY accounts.display_name, accounts.id",
            )?;
            let rows = statement.query_map(
                params![organization.to_string(), caller.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )?;
            let mut agents = Vec::new();
            for row in rows {
                let (account, name, email) = row?;
                agents.push(Agent {
                    account: parse_uuid(&account)?,
                    name,
                    email,
                });
            }
            Ok(agents)
        })
        .await
    }

    pub async fn remove_agent(
        &self,
        caller: Uuid,
        organization: Uuid,
        agent: Uuid,
    ) -> Result<Vec<crate::contacts::Granted>, ServerError> {
        self.membership(caller, organization).await?;
        let removed = self
            .with_graph(organization, move |_, graph, database| {
                require_agent_of(graph, database, caller, organization, agent)?;
                let transaction = write_transaction(database)?;
                let revoked = delete_agent(graph, &transaction, organization, agent)?;
                transaction.commit()?;
                Ok(revoked)
            })
            .await;
        self.sessions_changed();
        removed
    }

    /// Issues `agent`, one of `caller`'s, a new token and revokes every older one, so a copy
    /// of an older one no longer signs in and its connections close.
    pub async fn rotate_agent_token(
        &self,
        caller: Uuid,
        organization: Uuid,
        agent: Uuid,
    ) -> Result<String, ServerError> {
        self.membership(caller, organization).await?;
        let token = self
            .with_graph(organization, move |_, graph, database| {
                require_agent_of(graph, database, caller, organization, agent)?;
                let transaction = write_transaction(database)?;
                transaction.execute(
                    "DELETE FROM sessions WHERE account_id = ?1",
                    [agent.to_string()],
                )?;
                let token = issue_token(&transaction, agent)?;
                transaction.commit()?;
                Ok(token)
            })
            .await?;
        self.sessions_changed();
        Ok(token)
    }

    /// The organization and block `account` is a block's agent of, if it is one.
    pub async fn bound_to(&self, account: Uuid) -> Result<Option<(Uuid, Uuid)>, ServerError> {
        self.with_database(move |_, database| {
            let row: Option<(String, String)> = database
                .query_row(
                    "SELECT organization_id, bound_to FROM agents
                     WHERE account_id = ?1 AND bound_to IS NOT NULL",
                    [account.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            row.map(|(organization, block)| Ok((parse_uuid(&organization)?, parse_uuid(&block)?)))
                .transpose()
        })
        .await
    }

    /// Refuses every call of a block's agent while its block is in the trash; Restore lets them
    /// through again.
    pub async fn require_bound_live(
        &self,
        organization: Uuid,
        block: Uuid,
    ) -> Result<(), ServerError> {
        let live = match self.read_graph(organization, |graph| graph.is_live(block)) {
            Some(live) => live,
            None => {
                self.with_graph(organization, move |_, graph, _| Ok(graph.is_live(block)))
                    .await?
            }
        };
        match live {
            true => Ok(()),
            false => Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "the block this agent is bound to is in the trash".into(),
            )),
        }
    }

    pub async fn list_members(
        &self,
        identity: Identity,
    ) -> Result<Vec<be_block_protocol::Member>, ServerError> {
        require_administrator(identity)?;
        self.with_database(move |_, database| {
            let contacts = crate::contacts::contacts_of(&database, identity.account)?;
            let mut statement = database.prepare(
                "SELECT accounts.id, CASE WHEN accounts.local_profile THEN '' ELSE accounts.email END, accounts.display_name, memberships.role,
                        agents.account_id IS NOT NULL
                 FROM memberships
                 JOIN accounts ON accounts.id = memberships.account_id
                 LEFT JOIN agents ON agents.account_id = memberships.account_id
                    AND agents.organization_id = memberships.organization_id
                 WHERE memberships.organization_id = ?1
                 ORDER BY accounts.display_name, accounts.email",
            )?;
            let rows = statement.query_map([identity.organization.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, bool>(4)?,
                ))
            })?;
            let mut members = Vec::new();
            for row in rows {
                let (account, email, display_name, role, agent) = row?;
                let account = parse_uuid(&account)?;
                members.push(be_block_protocol::Member {
                    account,
                    email,
                    display_name,
                    role: decode_role(&role)?,
                    agent,
                    contact: contacts.contains(&account),
                });
            }
            Ok(members)
        })
        .await
    }

    pub async fn set_role(
        &self,
        identity: Identity,
        account: Uuid,
        role: OrganizationRole,
    ) -> Result<(), ServerError> {
        require_administrator(identity)?;
        let organization = identity.organization;
        self.with_graph(organization, move |store, graph, database| {
            let current = crate::contacts::role_in(database, account, organization)?
                .ok_or_else(not_a_member)?;
            if current == role {
                return Ok(());
            }
            if is_agent(database, account)? {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "an agent's role does not change".into(),
                ));
            }
            if current == OrganizationRole::Administrator {
                refuse_last_administrator(database, organization)?;
            }
            let transaction = write_transaction(database)?;
            transaction.execute(
                "UPDATE memberships SET role = ?3 WHERE organization_id = ?1 AND account_id = ?2",
                params![
                    organization.to_string(),
                    account.to_string(),
                    encode_role(role)
                ],
            )?;
            if current == OrganizationRole::Administrator {
                crate::keys::rotate_escrow_later(&transaction, organization, account)?;
            }
            let forgotten = forget_orphaned_block_agents(graph, &transaction, organization)?;
            transaction.commit()?;
            if !forgotten.is_empty() {
                store.sessions_changed();
            }
            Ok(())
        })
        .await
    }

    pub async fn remove_member(
        &self,
        identity: Identity,
        account: Uuid,
    ) -> Result<Vec<crate::contacts::Granted>, ServerError> {
        require_administrator(identity)?;
        let organization = identity.organization;
        self.with_graph(organization, move |store, graph, database| {
            let role = crate::contacts::role_in(database, account, organization)?
                .ok_or_else(not_a_member)?;
            if role == OrganizationRole::Administrator {
                refuse_last_administrator(database, organization)?;
            }
            let transaction = write_transaction(database)?;
            let revoked = remove_from(graph, &transaction, organization, account)?;
            let forgotten = forget_orphaned_block_agents(graph, &transaction, organization)?;
            transaction.commit()?;
            if !forgotten.is_empty() {
                store.sessions_changed();
            }
            Ok(revoked)
        })
        .await
    }

    pub async fn opened(
        &self,
        account: Uuid,
        organization: Uuid,
    ) -> Result<(OrganizationKind, Vec<be_block_protocol::EscrowKey>, Vec<u8>), ServerError> {
        self.with_database(move |_, database| {
            Ok((
                crate::keys::organization_kind(&database, organization)?,
                crate::keys::escrow_chain(&database, organization)?,
                crate::keys::pinned_escrow(&database, account, organization)?,
            ))
        })
        .await
    }

    pub async fn issue_session(&self, account: Uuid) -> Result<String, ServerError> {
        self.with_database(move |_, database| issue_token(&database, account))
            .await
    }

    pub async fn membership(
        &self,
        account: Uuid,
        organization: Uuid,
    ) -> Result<OrganizationRole, ServerError> {
        self.with_database(move |_, database| {
            let role: Option<String> = database
                .query_row(
                    "SELECT role FROM memberships WHERE organization_id = ?1 AND account_id = ?2",
                    params![organization.to_string(), account.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(role) = role else {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "that account is not a member of this organization".into(),
                ));
            };
            decode_role(&role)
        })
        .await
    }

    pub async fn add_member(
        &self,
        organization: Uuid,
        account: Uuid,
        role: OrganizationRole,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            database.execute(
                "INSERT OR REPLACE INTO memberships (organization_id, account_id, role)
                 VALUES (?1, ?2, ?3)",
                params![
                    organization.to_string(),
                    account.to_string(),
                    encode_role(role)
                ],
            )?;
            Ok(())
        })
        .await
    }

    /// Reads objects in order until the next would take more than `room` bytes, skipping any
    /// that are missing. They are read a batch at a time (`Objects::get_many`), so a bucket
    /// costs a round trip per batch rather than per object; a batch may read past the last
    /// object that fits.
    pub async fn get_objects(
        &self,
        hashes: Vec<Hash>,
        mut room: usize,
    ) -> Result<Vec<Vec<u8>>, ServerError> {
        let store = self.objects.clone();
        blocking(move || {
            let mut objects = Vec::new();
            for batch in hashes.chunks(crate::objects::PARALLEL_READS) {
                for read in store.get_many(batch) {
                    let Some(bytes) = read? else {
                        continue;
                    };
                    if bytes.len() > room {
                        return Ok(objects);
                    }
                    room -= bytes.len();
                    objects.push(bytes);
                }
            }
            Ok(objects)
        })
        .await
    }
}

/// Runs file work on tokio's blocking pool.
pub(crate) async fn blocking<R: Send + 'static>(
    work: impl FnOnce() -> Result<R, ServerError> + Send + 'static,
) -> Result<R, ServerError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ServerError::Io(std::io::Error::other(error)))?
}

/// Whether the server holds `hash`: `object_sizes` names every object from when its upload is
/// reserved until it is forgotten, and `confirmed` once its bytes were seen in the object store
/// (`ServerStore::confirm_uploads`), so this asks no object store.
pub(crate) fn stored(connection: &Connection, hash: Hash) -> Result<bool, ServerError> {
    Ok(connection
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM object_sizes WHERE hash = ?1 AND confirmed)")?
        .query_row([hash.to_hex()], |row| row.get(0))?)
}

/// Whether `identity` uploaded `hash` in its organization and proved it has the object's bytes
/// (`ServerStore::reserve_uploads`).
pub(crate) fn uploaded(
    connection: &Connection,
    identity: Identity,
    hash: Hash,
) -> Result<bool, ServerError> {
    Ok(connection
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM object_uploads
             WHERE organization_id = ?1 AND account_id = ?2 AND hash = ?3 AND proven)",
        )?
        .query_row(
            params![
                identity.organization.to_string(),
                identity.account.to_string(),
                hash.to_hex()
            ],
            |row| row.get(0),
        )?)
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
    let valid = email.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty()
            && local.len() <= 64
            && !domain.is_empty()
            && !domain.contains('@')
            && !domain.split('.').any(str::is_empty)
    });
    if !valid || email.len() > 254 || email.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            "Email: enter a valid email address of at most 254 bytes.".into(),
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

fn dummy_hash() -> &'static str {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password(&Uuid::new_v4().to_string()).expect("hashing a random password succeeds")
    })
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
        "INSERT INTO sessions (token_hash, account_id, kind, public_id, created) VALUES (?1, ?2, 'app', ?3, ?4)",
        params![hash_token(&token), account.to_string(), Uuid::new_v4().to_string(), portal::now()],
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

fn personal_organization(database: &Connection) -> Result<Uuid, ServerError> {
    let found: Option<String> = database
        .query_row(
            "SELECT id FROM organizations WHERE kind = 'personal' ORDER BY id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = found {
        return parse_uuid(&id);
    }
    let id = Uuid::new_v4();
    database.execute(
        "INSERT INTO organizations (id, name, owner_id, kind) VALUES (?1, 'Personal', NULL, 'personal')",
        [id.to_string()],
    )?;
    Ok(id)
}

pub fn decode_kind(value: &str) -> Result<OrganizationKind, ServerError> {
    match value {
        "company" => Ok(OrganizationKind::Company),
        "personal" => Ok(OrganizationKind::Personal),
        _ => Err(ServerError::Corrupt),
    }
}

pub fn decode_role(value: &str) -> Result<OrganizationRole, ServerError> {
    match value {
        "administrator" => Ok(OrganizationRole::Administrator),
        "editor" => Ok(OrganizationRole::Editor),
        _ => Err(ServerError::Corrupt),
    }
}

pub fn encode_role(role: OrganizationRole) -> &'static str {
    match role {
        OrganizationRole::Administrator => "administrator",
        OrganizationRole::Editor => "editor",
    }
}

pub fn encode_access(access: Access) -> Option<&'static str> {
    match access {
        Access::None => None,
        Access::KnowExists => Some("know_exists"),
        Access::View => Some("view"),
        Access::Edit => Some("edit"),
        Access::Manage => Some("manage"),
    }
}

pub fn decode_access(value: &str) -> Result<Access, ServerError> {
    match value {
        "know_exists" => Ok(Access::KnowExists),
        "view" => Ok(Access::View),
        "edit" => Ok(Access::Edit),
        "manage" => Ok(Access::Manage),
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

/// An organization's graph as the store caches it. A graph job that may have left it apart from
/// the database marks it invalid before letting go of it, so a reader that already holds it
/// does not trust it either.
struct Cached {
    graph: BlockGraph,
    valid: bool,
}

pub struct Collected {
    pub blocks: Vec<Uuid>,
    pub objects: usize,
    pub revision: u64,
}

impl ServerStore {
    /// Advances the revision counter on every call, under the lock every graph read and change
    /// takes, so a larger revision is always a later view. Clients order out-of-order replies
    /// and announcements by it, and a listing kept offline by the revision it was read at. It
    /// only grows, across restarts too: `revision_floor` holds where the next start begins.
    pub(crate) async fn with_graph<R: Send + 'static>(
        &self,
        organization: Uuid,
        action: impl FnOnce(&ServerStore, &mut BlockGraph, &Connection) -> Result<R, ServerError>
        + Send
        + 'static,
    ) -> Result<R, ServerError> {
        self.with_database(move |store, database| {
            store.in_graph(organization, database, |graph| {
                action(store, graph, database)
            })
        })
        .await
    }

    /// Runs `job` with the database, alone, on tokio's blocking pool. A job runs to its end even
    /// when the call that started it is dropped, so a transaction is never cut short.
    pub(crate) async fn with_database<R: Send + 'static>(
        &self,
        job: impl FnOnce(&ServerStore, &Connection) -> Result<R, ServerError> + Send + 'static,
    ) -> Result<R, ServerError> {
        let database = Arc::clone(&self.database).lock_owned().await;
        let store = self.clone();
        blocking(move || job(&store, &database)).await
    }

    fn in_graph<R>(
        &self,
        organization: Uuid,
        database: &Connection,
        action: impl FnOnce(&mut BlockGraph) -> Result<R, ServerError>,
    ) -> Result<R, ServerError> {
        #[cfg(test)]
        held::wait(organization);
        let cached = {
            let mut graphs = self
                .graphs
                .lock()
                .expect("the graph cache is never poisoned");
            match graphs.entry(organization) {
                Entry::Occupied(entry) => Arc::clone(entry.get()),
                Entry::Vacant(entry) => {
                    self.graph_loads
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let graph = load_graph(database, organization)?;
                    Arc::clone(entry.insert(Arc::new(std::sync::RwLock::new(Cached {
                        graph,
                        valid: true,
                    }))))
                }
            }
        };
        let mut held = cached.write().expect("a cached graph is never poisoned");
        let graph = &mut held.graph;
        let revision = self
            .revision
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        if revision >= self.reserved.load(std::sync::atomic::Ordering::Relaxed) {
            let reserved = reserve_revisions(database, Some(revision))? + REVISIONS_RESERVED;
            self.reserved
                .fetch_max(reserved, std::sync::atomic::Ordering::Relaxed);
        }
        graph.advance_to(revision);
        let changes = (graph.changes(), total_changes(database)?);
        let outcome = action(graph);
        // A refusal that changed nothing leaves the cache as it was, so a stream of refused
        // calls costs no reloads; anything else may have left the graph and the database apart.
        let kept = match &outcome {
            Ok(_) => true,
            Err(ServerError::Refused(..)) => changes == (graph.changes(), total_changes(database)?),
            Err(_) => false,
        };
        if !kept {
            held.valid = false;
            let mut graphs = self
                .graphs
                .lock()
                .expect("the graph cache is never poisoned");
            if graphs
                .get(&organization)
                .is_some_and(|current| Arc::ptr_eq(current, &cached))
            {
                graphs.remove(&organization);
            }
        }
        outcome
    }

    /// Reads `organization`'s cached graph without the database lock, waiting only for a graph
    /// job of the same organization; `None` when it is not cached.
    pub(crate) fn read_graph<R>(
        &self,
        organization: Uuid,
        read: impl FnOnce(&BlockGraph) -> R,
    ) -> Option<R> {
        let cached = self
            .graphs
            .lock()
            .expect("the graph cache is never poisoned")
            .get(&organization)
            .cloned()?;
        let held = cached.read().expect("a cached graph is never poisoned");
        held.valid.then(|| read(&held.graph))
    }

    /// Runs `read` with the read-only connection on tokio's blocking pool.
    pub(crate) async fn with_reader<R: Send + 'static>(
        &self,
        read: impl FnOnce(&ServerStore, &Connection) -> Result<R, ServerError> + Send + 'static,
    ) -> Result<R, ServerError> {
        let store = self.clone();
        blocking(move || {
            let reader = store.reader.lock().expect("the reader is never poisoned");
            read(&store, &reader)
        })
        .await
    }

    /// How many times an organization's graph was loaded from the database.
    #[cfg(test)]
    pub(crate) fn graph_loads(&self) -> u64 {
        self.graph_loads.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Runs `job`, which changes the in-memory `ObjectRefs` beside a transaction of its own.
    /// When it fails its transaction rolled back, so the counts are read again from the
    /// database, which holds what was committed.
    pub(crate) fn keeping_refs<R>(
        &self,
        database: &Connection,
        job: impl FnOnce() -> Result<R, ServerError>,
    ) -> Result<R, ServerError> {
        let result = job();
        if result.is_err() {
            match load_object_refs(database) {
                Ok(refs) => *self.object_refs() = refs,
                Err(error) => tracing::error!(%error, "reading object counts again failed"),
            }
        }
        result
    }

    pub(crate) fn object_refs(&self) -> std::sync::MutexGuard<'_, ObjectRefs> {
        self.object_refs
            .lock()
            .expect("object refs are never poisoned")
    }

    pub fn objects(&self) -> &crate::objects::Objects {
        &self.objects
    }

    pub(crate) fn server_key(&self) -> &crate::organization::ServerKey {
        &self.server
    }

    pub fn server_account(&self) -> be_block_protocol::ServerAccount {
        self.server.described()
    }

    pub(crate) fn policy_cache(
        &self,
    ) -> &std::sync::Mutex<HashMap<Uuid, crate::organization::HeldPolicies>> {
        &self.policies
    }
}

fn is_member(
    connection: &Connection,
    account: Uuid,
    organization: Uuid,
) -> Result<bool, ServerError> {
    Ok(connection
        .query_row(
            "SELECT 1 FROM memberships WHERE organization_id = ?1 AND account_id = ?2",
            params![organization.to_string(), account.to_string()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn require_administrator(identity: Identity) -> Result<(), ServerError> {
    match identity.is_administrator() {
        true => Ok(()),
        false => Err(ServerError::Refused(
            ErrorCode::PermissionDenied,
            "only an organization administrator can manage its members".into(),
        )),
    }
}

fn refuse_last_administrator(
    connection: &Connection,
    organization: Uuid,
) -> Result<(), ServerError> {
    let administrators: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memberships WHERE organization_id = ?1 AND role = 'administrator'",
        [organization.to_string()],
        |row| row.get(0),
    )?;
    match administrators > 1 {
        true => Ok(()),
        false => Err(ServerError::Refused(
            ErrorCode::LastAdministrator,
            "an organization keeps at least one administrator".into(),
        )),
    }
}

fn remove_from(
    graph: &mut BlockGraph,
    connection: &Connection,
    organization: Uuid,
    account: Uuid,
) -> Result<Vec<crate::contacts::Granted>, ServerError> {
    let role = crate::contacts::role_in(connection, account, organization)?;
    let mut revoked = Vec::new();
    let mut kinds = std::collections::BTreeSet::new();
    for block in graph.entry_points(account) {
        kinds.extend(crate::keys::open_kinds_viewed(
            connection,
            graph,
            organization,
            account,
            block,
        )?);
    }
    for block in graph.entry_points(account) {
        let Some(node) = graph.get(block) else {
            continue;
        };
        let held = node.grants.contains_key(&account);
        let shared = node.grants.keys().any(|other| *other != account);
        let before = graph.audience(block);
        if held {
            crate::blocks::write_grant(
                graph,
                connection,
                organization,
                block,
                account,
                Access::None,
            )?;
        }
        if held || shared {
            crate::keys::lost_view(connection, organization, block, account)?;
            revoked.push(crate::contacts::Granted {
                organization,
                block,
                before,
            });
        }
    }
    connection.execute(
        "DELETE FROM memberships WHERE organization_id = ?1 AND account_id = ?2",
        params![organization.to_string(), account.to_string()],
    )?;
    crate::notifications::forget_member(connection, organization, account)?;
    connection.execute(
        "DELETE FROM wrapped_keys WHERE organization_id = ?1 AND wrapped_by = ?2",
        params![
            organization.to_string(),
            crate::keys::encode_wrapped_by(be_block_protocol::WrappedBy::Account(account))
        ],
    )?;
    if role == Some(OrganizationRole::Administrator) {
        crate::keys::rotate_escrow_later(connection, organization, account)?;
    }
    let settings = crate::organization::settings_of(organization);
    if graph.contains(settings) {
        crate::keys::lost_view(connection, organization, settings, account)?;
    }
    crate::keys::lost_open_kinds(connection, graph, organization, account, kinds)?;
    Ok(revoked)
}

/// Refuses unless `agent` is one `caller` made in `organization` and, for a block's agent, the
/// caller still manages its block.
fn require_agent_of(
    graph: &BlockGraph,
    connection: &Connection,
    caller: Uuid,
    organization: Uuid,
    agent: Uuid,
) -> Result<(), ServerError> {
    let found: Option<Option<String>> = connection
        .query_row(
            "SELECT bound_to FROM agents WHERE account_id = ?1 AND organization_id = ?2 AND created_by = ?3",
            params![agent.to_string(), organization.to_string(), caller.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(bound) = found else {
        return Err(ServerError::Refused(
            ErrorCode::AgentNotFound,
            "that account is not an agent of this organization".into(),
        ));
    };
    match bound.as_deref().map(parse_uuid).transpose()? {
        Some(block) if !manages(graph, connection, caller, organization, block)? => {
            Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                format!("block {block} is not managed by this account"),
            ))
        }
        _ => Ok(()),
    }
}

/// Whether `account`, as a member of `organization`, manages `block`.
fn manages(
    graph: &BlockGraph,
    connection: &Connection,
    account: Uuid,
    organization: Uuid,
    block: Uuid,
) -> Result<bool, ServerError> {
    Ok(
        crate::contacts::role_in(connection, account, organization)?.is_some_and(|role| {
            crate::blocks::visibility(Identity {
                account,
                organization,
                role,
                bound: None,
            })
            .can_manage(graph, block)
        }),
    )
}

fn is_agent(connection: &Connection, account: Uuid) -> Result<bool, ServerError> {
    Ok(connection
        .query_row(
            "SELECT 1 FROM agents WHERE account_id = ?1",
            [account.to_string()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Deletes an agent's account, with what it was granted and what was sealed to it.
fn delete_agent(
    graph: &mut BlockGraph,
    connection: &Connection,
    organization: Uuid,
    agent: Uuid,
) -> Result<Vec<crate::contacts::Granted>, ServerError> {
    let revoked = remove_from(graph, connection, organization, agent)?;
    connection.execute("DELETE FROM accounts WHERE id = ?1", [agent.to_string()])?;
    connection.execute(
        "DELETE FROM wrapped_keys WHERE wrapped_by = ?1",
        [crate::keys::encode_wrapped_by(
            be_block_protocol::WrappedBy::Account(agent),
        )],
    )?;
    Ok(revoked)
}

/// Deletes the blocks' agents of `organization` whose block was collected, who lost their grant
/// on it, or whose creator no longer manages it, after a change that may have done any of
/// these (a grant, a move, a role, a member or contact removed, a collection); answers them. The caller tells
/// `ServerStore::sessions_changed` once its transaction commits, which closes their
/// connections.
/// Whether moving `moved` put some block's agent's block in the trash, whose connections
/// then close (`Connection::signed_in`).
pub(crate) fn trashes_block_agents(
    graph: &BlockGraph,
    connection: &Connection,
    organization: Uuid,
    moved: Uuid,
) -> Result<bool, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT bound_to FROM agents WHERE organization_id = ?1 AND bound_to IS NOT NULL",
    )?;
    let rows = statement.query_map([organization.to_string()], |row| row.get::<_, String>(0))?;
    for row in rows {
        let block = parse_uuid(&row?)?;
        let under = block == moved || graph.ancestors(block).any(|above| above == moved);
        if under && !graph.is_live(block) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn forget_orphaned_block_agents(
    graph: &mut BlockGraph,
    connection: &Connection,
    organization: Uuid,
) -> Result<Vec<Uuid>, ServerError> {
    let bound: Vec<(Uuid, Uuid, Uuid)> = {
        let mut statement = connection.prepare(
            "SELECT account_id, bound_to, created_by FROM agents
             WHERE organization_id = ?1 AND bound_to IS NOT NULL",
        )?;
        let rows = statement.query_map([organization.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut bound = Vec::new();
        for row in rows {
            let (agent, block, creator) = row?;
            bound.push((
                parse_uuid(&agent)?,
                parse_uuid(&block)?,
                parse_uuid(&creator)?,
            ));
        }
        bound
    };
    let mut forgotten = Vec::new();
    for (agent, block, creator) in bound {
        let granted = graph
            .get(block)
            .and_then(|node| node.grants.get(&agent))
            .is_some_and(|access| *access > Access::None);
        // An agent reaches no further than its creator: one whose creator no longer manages
        // its block would keep the creator an editor there. In the trash it waits for Restore.
        let orphaned =
            graph.is_live(block) && !manages(graph, connection, creator, organization, block)?;
        if !granted || orphaned {
            delete_agent(graph, connection, organization, agent)?;
            forgotten.push(agent);
        }
    }
    Ok(forgotten)
}

fn not_a_member() -> ServerError {
    ServerError::Refused(
        ErrorCode::PermissionDenied,
        "that account is not a member of this organization".into(),
    )
}

fn recovery_key(connection: &Connection, account: Uuid) -> Result<Option<[u8; 32]>, ServerError> {
    connection
        .query_row(
            "SELECT public FROM recovery_keys WHERE account_id = ?1",
            [account.to_string()],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()?
        .map(|public| public_key(&public))
        .transpose()
}

fn public_key(bytes: &[u8]) -> Result<[u8; 32], ServerError> {
    bytes.try_into().map_err(|_| ServerError::Corrupt)
}

pub(crate) fn recovery_key_of(
    connection: &Connection,
    account: Uuid,
) -> Result<Option<[u8; 32]>, ServerError> {
    recovery_key(connection, account)
}

pub(crate) fn is_member_of(
    connection: &Connection,
    account: Uuid,
    organization: Uuid,
) -> Result<bool, ServerError> {
    is_member(connection, account, organization)
}

/// Rows any statement on `connection` changed, counting those a rollback undid.
fn total_changes(connection: &Connection) -> Result<i64, ServerError> {
    Ok(connection
        .prepare_cached("SELECT total_changes()")?
        .query_row([], |row| row.get(0))?)
}

pub(crate) fn load_graph(
    connection: &Connection,
    organization: Uuid,
) -> Result<BlockGraph, ServerError> {
    let mut graph = BlockGraph::new();
    let mut nodes = Vec::new();
    {
        let mut statement = connection.prepare(
            "SELECT id, content_type, author, parent_kind, parent_id, head, metadata, version, key_id,
                owner_only
             FROM blocks WHERE organization_id = ?1",
        )?;
        let rows = statement.query_map([organization.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Vec<u8>>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, bool>(9)?,
            ))
        })?;
        for row in rows {
            nodes.push(row?);
        }
    }
    for (
        id,
        content_type,
        author,
        parent_kind,
        parent_id,
        head,
        metadata,
        version,
        key,
        owner_only,
    ) in &nodes
    {
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
        node.key = match key.is_empty() {
            true => None,
            false => Some(parse_uuid(key)?),
        };
        node.owner_only = *owner_only;
        let _ = (parent_kind, parent_id);
        graph.insert(parse_uuid(id)?, node)?;
    }
    for (id, _, _, parent_kind, parent_id, _, _, _, _, _) in &nodes {
        let parent = decode_parent(*parent_kind, parent_id.clone())?;
        graph.set_parent(parse_uuid(id)?, parent)?;
    }
    {
        let mut statement = connection.prepare(
            "SELECT block_id, reference_id FROM block_edges WHERE organization_id = ?1
                 ORDER BY block_id, position, rowid",
        )?;
        let rows = statement.query_map([organization.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut edges: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        for row in rows {
            let (block, reference) = row?;
            edges
                .entry(parse_uuid(&block)?)
                .or_default()
                .push(parse_uuid(&reference)?);
        }
        for (block, references) in edges {
            graph.set_references(block, &references)?;
        }
    }
    {
        let mut statement = connection.prepare(
            "SELECT block_id, account_id, access FROM block_access WHERE organization_id = ?1",
        )?;
        let rows = statement.query_map([organization.to_string()], |row| {
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
    usage::load_sizes(connection, &mut graph, organization)?;
    Ok(graph)
}
