pub(crate) mod portal;
pub use portal::{AccountSession, MailLink};
use std::{
    collections::{BTreeMap, HashMap, hash_map::Entry},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use be_commit::CommitId;
use be_graph::{Access, BlockGraph, BlockNode, BlockParent, GraphError, ObjectRefs};
use be_protocol::{
    Agent, ErrorCode, Organization, OrganizationInvitation, OrganizationKind, OrganizationRole,
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
    pub organization: Uuid,
    pub role: OrganizationRole,
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

pub struct ServerStore {
    database: Mutex<Connection>,
    pub(crate) session_changes: tokio::sync::watch::Sender<u64>,
    objects: FileStore,
    graphs: Mutex<HashMap<Uuid, BlockGraph>>,
    revision: std::sync::atomic::AtomicU64,
    object_refs: std::sync::Mutex<ObjectRefs>,
    hashing: tokio::sync::Semaphore,
    failures: std::sync::Mutex<HashMap<String, Failures>>,
    server: crate::organization::ServerKey,
    policies: std::sync::Mutex<HashMap<Uuid, crate::organization::HeldPolicies>>,
}

struct Failures {
    count: u32,
    until: Option<Instant>,
}

const PASSWORD_CHECKS_AT_ONCE: usize = 4;
const FREE_FAILURES: u32 = 5;
const FIRST_LOCKOUT: Duration = Duration::from_secs(30);
const LONGEST_LOCKOUT: Duration = Duration::from_secs(60 * 60);

impl ServerStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ServerError> {
        let root: PathBuf = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        let connection = Connection::open(root.join("metadata.sqlite"))?;
        connection.busy_timeout(Duration::from_secs(5))?;
        schema::initialize(&connection)?;
        connection.execute("UPDATE sessions SET kind = 'app', public_id = lower(hex(randomblob(16))), created = ?1 WHERE kind = 'legacy' AND (account_id IN (SELECT account_id FROM agents) OR account_id IN (SELECT id FROM accounts WHERE local_profile = 1))", [portal::now()])?;
        let object_refs = load_object_refs(&connection)?;
        let server = crate::organization::ServerKey::load_or_create(&connection)?;
        Ok(Self {
            database: Mutex::new(connection),
            session_changes: tokio::sync::watch::channel(0).0,
            objects: FileStore::open(root.join("objects"))?,
            graphs: Mutex::new(HashMap::new()),
            revision: std::sync::atomic::AtomicU64::new(0),
            object_refs: std::sync::Mutex::new(object_refs),
            hashing: tokio::sync::Semaphore::new(PASSWORD_CHECKS_AT_ONCE),
            failures: std::sync::Mutex::new(HashMap::new()),
            server,
            policies: std::sync::Mutex::new(HashMap::new()),
        })
    }

    pub async fn create_local_profile(
        &self,
        display_name: &str,
    ) -> Result<(Profile, String), ServerError> {
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "a display name is required".into(),
            ));
        }
        let account = Uuid::new_v4();
        let database = self.database.lock().await;
        let transaction = database.unchecked_transaction()?;
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
        let hash = self.hash(password.to_owned()).await?;
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
        let transaction = database.unchecked_transaction()?;
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
    }

    pub async fn login_browser(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(Profile, String), ServerError> {
        let email = normalize_email(email)?;
        if let Some(wait) = self.locked_out(&email) {
            return Err(ServerError::Refused(
                ErrorCode::InvalidCredentials,
                format!(
                    "too many failed sign-ins for this account; try again in {} seconds",
                    wait.as_secs().max(1)
                ),
            ));
        }
        let row: Option<(String, String, String)> = {
            let database = self.database.lock().await;
            database
                .query_row(
                    "SELECT id, display_name, password_hash FROM accounts WHERE email = ?1",
                    [&email],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?
        };
        let hash = row
            .as_ref()
            .map_or_else(|| dummy_hash().to_owned(), |(_, _, hash)| hash.clone());
        let matches = self.verify(password.to_owned(), hash).await?;
        let Some((id, display_name, previous_hash)) = row.filter(|_| matches) else {
            self.failed(&email);
            return Err(ServerError::Refused(
                ErrorCode::InvalidCredentials,
                "no account matches those credentials".into(),
            ));
        };
        self.failures.lock().unwrap().remove(&email);
        let account = parse_uuid(&id)?;
        let database = self.database.lock().await;
        let transaction = database.unchecked_transaction()?;
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
    }

    fn locked_out(&self, email: &str) -> Option<Duration> {
        let failures = self.failures.lock().unwrap();
        let until = failures.get(email)?.until?;
        until.checked_duration_since(Instant::now())
    }

    fn failed(&self, email: &str) {
        let mut failures = self.failures.lock().unwrap();
        let entry = failures.entry(email.to_owned()).or_insert(Failures {
            count: 0,
            until: None,
        });
        entry.count += 1;
        if entry.count >= FREE_FAILURES {
            let doublings = (entry.count - FREE_FAILURES).min(16);
            let wait = FIRST_LOCKOUT
                .saturating_mul(1 << doublings)
                .min(LONGEST_LOCKOUT);
            entry.until = Some(Instant::now() + wait);
            tracing::warn!(email, failures = entry.count, ?wait, "sign-ins locked out");
        }
    }

    async fn hash(&self, password: String) -> Result<String, ServerError> {
        let _permit = self
            .hashing
            .acquire()
            .await
            .map_err(|_| ServerError::Corrupt)?;
        tokio::task::spawn_blocking(move || hash_password(&password))
            .await
            .map_err(|_| ServerError::Corrupt)?
    }

    async fn verify(&self, password: String, hash: String) -> Result<bool, ServerError> {
        let _permit = self
            .hashing
            .acquire()
            .await
            .map_err(|_| ServerError::Corrupt)?;
        tokio::task::spawn_blocking(move || verify_password(&password, &hash))
            .await
            .map_err(|_| ServerError::Corrupt)
    }

    pub async fn checkpoint(&self) -> Result<(), ServerError> {
        self.database
            .lock()
            .await
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    pub async fn resolve_token(&self, token: &str) -> Result<Profile, ServerError> {
        let database = self.database.lock().await;
        let row: Option<(String, String, String)> = database
            .query_row(
                "SELECT accounts.id, CASE WHEN accounts.local_profile THEN '' ELSE accounts.email END, accounts.display_name FROM sessions
                 JOIN accounts ON accounts.id = sessions.account_id
                 WHERE sessions.token_hash = ?1 AND sessions.kind = 'app'",
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
            "SELECT CASE WHEN local_profile THEN '' ELSE email END, display_name FROM accounts WHERE id = ?1",
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
        self.sessions_changed();
        Ok(())
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
        let database = self.database.lock().await;
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
    }

    pub async fn invitation(
        &self,
        account: Uuid,
        invitation: Uuid,
    ) -> Result<OrganizationInvitation, ServerError> {
        let email = self.profile(account).await?.email;
        let database = self.database.lock().await;
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
    }

    pub async fn accept_invitation(
        &self,
        account: Uuid,
        invitation: Uuid,
        card: Vec<u8>,
        contact: Vec<u8>,
        escrow: Vec<u8>,
    ) -> Result<Uuid, ServerError> {
        let found = self.invitation(account, invitation).await?;
        let database = self.database.lock().await;
        let transaction = database.unchecked_transaction()?;
        let code: Vec<u8> = transaction.query_row(
            "SELECT code FROM invitations WHERE id = ?1",
            [invitation.to_string()],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT OR REPLACE INTO contact_links (id, owner_id, card, code, accepter_id, accepter_card)
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
    }

    pub async fn list_organizations(
        &self,
        account: Uuid,
    ) -> Result<Vec<Organization>, ServerError> {
        let database = self.database.lock().await;
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
    }

    pub async fn create_organization(
        &self,
        account: Uuid,
        name: &str,
    ) -> Result<Organization, ServerError> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "an organization name is required".into(),
            ));
        }
        let id = Uuid::new_v4();
        let database = self.database.lock().await;
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
    }

    pub async fn personal_organization(&self) -> Result<Uuid, ServerError> {
        let database = self.database.lock().await;
        personal_organization(&database)
    }

    pub async fn join_personal_organization(&self, account: Uuid) -> Result<Uuid, ServerError> {
        let database = self.database.lock().await;
        let organization = personal_organization(&database)?;
        database.execute(
            "INSERT OR IGNORE INTO memberships (organization_id, account_id, role)
             VALUES (?1, ?2, 'editor')",
            params![organization.to_string(), account.to_string()],
        )?;
        Ok(organization)
    }

    pub async fn create_agent(
        &self,
        caller: Uuid,
        organization: Uuid,
        name: &str,
        recovery: [u8; 32],
        contact: Vec<u8>,
        escrow: Vec<u8>,
    ) -> Result<(Agent, String), ServerError> {
        self.membership(caller, organization).await?;
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "an agent needs a name".into(),
            ));
        }
        let account = Uuid::new_v4();
        let email = format!("agent-{}@agents.invalid", account.simple());
        let database = self.database.lock().await;
        let transaction = database.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO accounts (id, email, display_name, password_hash) VALUES (?1, ?2, ?3, '!')",
            params![account.to_string(), email, name],
        )?;
        transaction.execute(
            "INSERT INTO agents (account_id, organization_id, created_by) VALUES (?1, ?2, ?3)",
            params![
                account.to_string(),
                organization.to_string(),
                caller.to_string()
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
            "INSERT INTO recovery_keys (account_id, public) VALUES (?1, ?2)",
            params![account.to_string(), recovery.to_vec()],
        )?;
        crate::contacts::befriend(&transaction, caller, &[], account, &contact)?;
        crate::keys::pin_escrow(&transaction, account, organization, &escrow)?;
        let token = issue_token(&transaction, account)?;
        transaction.commit()?;
        Ok((
            Agent {
                account,
                name,
                email,
            },
            token,
        ))
    }

    pub async fn list_agents(
        &self,
        caller: Uuid,
        organization: Uuid,
    ) -> Result<Vec<Agent>, ServerError> {
        let database = self.database.lock().await;
        if !is_member(&database, caller, organization)? {
            return Err(not_a_member());
        }
        let mut statement = database.prepare(
            "SELECT accounts.id, accounts.display_name, accounts.email FROM agents
             JOIN accounts ON accounts.id = agents.account_id
             WHERE agents.organization_id = ?1 AND agents.created_by = ?2
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
    }

    pub async fn remove_agent(
        &self,
        caller: Uuid,
        organization: Uuid,
        agent: Uuid,
    ) -> Result<Vec<crate::contacts::Granted>, ServerError> {
        self.membership(caller, organization).await?;
        self.with_graph(organization, move |graph, database| {
            let is_agent = database
                .query_row(
                    "SELECT 1 FROM agents WHERE account_id = ?1 AND organization_id = ?2 AND created_by = ?3",
                    params![agent.to_string(), organization.to_string(), caller.to_string()],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !is_agent {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "that account is not an agent of this organization".into(),
                ));
            }
            let transaction = database.unchecked_transaction()?;
            let revoked = remove_from(graph, &transaction, organization, agent)?;
            transaction.execute("DELETE FROM accounts WHERE id = ?1", [agent.to_string()])?;
            transaction.execute(
                "DELETE FROM wrapped_keys WHERE wrapped_by = ?1",
                [crate::keys::encode_wrapped_by(
                    be_protocol::WrappedBy::Account(agent),
                )],
            )?;
            transaction.commit()?;
            Ok(revoked)
        })
        .await
    }

    pub async fn list_members(
        &self,
        identity: Identity,
    ) -> Result<Vec<be_protocol::Member>, ServerError> {
        require_administrator(identity)?;
        let database = self.database.lock().await;
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
            members.push(be_protocol::Member {
                account,
                email,
                display_name,
                role: decode_role(&role)?,
                agent,
                contact: contacts.contains(&account),
            });
        }
        Ok(members)
    }

    pub async fn set_role(
        &self,
        identity: Identity,
        account: Uuid,
        role: OrganizationRole,
    ) -> Result<(), ServerError> {
        require_administrator(identity)?;
        let organization = identity.organization;
        let database = self.database.lock().await;
        let current =
            crate::contacts::role_in(&database, account, organization)?.ok_or_else(not_a_member)?;
        if current == role {
            return Ok(());
        }
        if current == OrganizationRole::Administrator {
            refuse_last_administrator(&database, organization)?;
        }
        let transaction = database.unchecked_transaction()?;
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
        transaction.commit()?;
        Ok(())
    }

    pub async fn remove_member(
        &self,
        identity: Identity,
        account: Uuid,
    ) -> Result<Vec<crate::contacts::Granted>, ServerError> {
        require_administrator(identity)?;
        let organization = identity.organization;
        self.with_graph(organization, move |graph, database| {
            let role = crate::contacts::role_in(database, account, organization)?
                .ok_or_else(not_a_member)?;
            if role == OrganizationRole::Administrator {
                refuse_last_administrator(database, organization)?;
            }
            let transaction = database.unchecked_transaction()?;
            let revoked = remove_from(graph, &transaction, organization, account)?;
            transaction.commit()?;
            Ok(revoked)
        })
        .await
    }

    pub async fn opened(
        &self,
        account: Uuid,
        organization: Uuid,
    ) -> Result<(OrganizationKind, Vec<be_protocol::EscrowKey>, Vec<u8>), ServerError> {
        let database = self.database.lock().await;
        Ok((
            crate::keys::organization_kind(&database, organization)?,
            crate::keys::escrow_chain(&database, organization)?,
            crate::keys::pinned_escrow(&database, account, organization)?,
        ))
    }

    pub async fn issue_session(&self, account: Uuid) -> Result<String, ServerError> {
        let database = self.database.lock().await;
        issue_token(&database, account)
    }

    pub async fn membership(
        &self,
        account: Uuid,
        organization: Uuid,
    ) -> Result<OrganizationRole, ServerError> {
        let database = self.database.lock().await;
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
    }

    pub async fn add_member(
        &self,
        organization: Uuid,
        account: Uuid,
        role: OrganizationRole,
    ) -> Result<(), ServerError> {
        let database = self.database.lock().await;
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
    }

    pub fn put_objects(&self, objects: &[impl AsRef<[u8]>]) -> Result<Vec<Hash>, ServerError> {
        if objects
            .iter()
            .any(|bytes| bytes.as_ref().len() > MAX_OBJECT_BYTES)
        {
            return Err(ServerError::Refused(
                ErrorCode::ObjectTooLarge,
                format!("an object may not exceed {MAX_OBJECT_BYTES} bytes"),
            ));
        }
        let hashes = objects
            .iter()
            .map(|bytes| self.objects.put(bytes.as_ref()))
            .collect::<Result<Vec<Hash>, _>>()?;
        self.objects.sync()?;
        Ok(hashes)
    }

    pub fn get_object(&self, hash: Hash) -> Result<Option<Vec<u8>>, ServerError> {
        Ok(self.objects.get(hash)?)
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

pub struct Collected {
    pub blocks: Vec<Uuid>,
    pub objects: usize,
    pub revision: u64,
}

impl ServerStore {
    pub(crate) async fn with_graph<R>(
        &self,
        organization: Uuid,
        action: impl FnOnce(&mut BlockGraph, &Connection) -> Result<R, ServerError>,
    ) -> Result<R, ServerError> {
        let database = self.database.lock().await;
        let mut graphs = self.graphs.lock().await;
        let graph = match graphs.entry(organization) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(load_graph(&database, organization)?),
        };
        let revision = self
            .revision
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        graph.advance_to(revision);
        let outcome = action(graph, &database);
        if outcome.is_err() {
            graphs.remove(&organization);
        }
        outcome
    }

    pub(crate) fn object_refs(&self) -> std::sync::MutexGuard<'_, ObjectRefs> {
        self.object_refs
            .lock()
            .expect("object refs are never poisoned")
    }

    pub(crate) fn database(&self) -> &Mutex<Connection> {
        &self.database
    }

    pub(crate) fn objects(&self) -> &FileStore {
        &self.objects
    }

    pub(crate) fn server_key(&self) -> &crate::organization::ServerKey {
        &self.server
    }

    pub fn server_account(&self) -> be_protocol::ServerAccount {
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
    connection.execute(
        "DELETE FROM wrapped_keys WHERE organization_id = ?1 AND wrapped_by = ?2",
        params![
            organization.to_string(),
            crate::keys::encode_wrapped_by(be_protocol::WrappedBy::Account(account))
        ],
    )?;
    if role == Some(OrganizationRole::Administrator) {
        crate::keys::rotate_escrow_later(connection, organization, account)?;
    }
    Ok(revoked)
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

pub(crate) fn load_graph(
    connection: &Connection,
    organization: Uuid,
) -> Result<BlockGraph, ServerError> {
    let mut graph = BlockGraph::new();
    let mut nodes = Vec::new();
    {
        let mut statement = connection.prepare(
            "SELECT id, content_type, author, parent_kind, parent_id, head, metadata, version, key_id
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
            ))
        })?;
        for row in rows {
            nodes.push(row?);
        }
    }
    for (id, content_type, author, parent_kind, parent_id, head, metadata, version, key) in &nodes {
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
        let _ = (parent_kind, parent_id);
        graph.insert(parse_uuid(id)?, node)?;
    }
    for (id, _, _, parent_kind, parent_id, _, _, _, _) in &nodes {
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
    Ok(graph)
}
