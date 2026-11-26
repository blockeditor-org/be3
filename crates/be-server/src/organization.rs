use be_block::{
    OrganizationPolicies, OrganizationPoliciesContent, SettingsContent, WORKSPACE_TYPE, WhoMay,
};
use be_block_commit::{CommitId, CommitStore};
use be_block_content::{BlockContent, Root};
use be_block_graph::{BlockGraph, BlockParent, SETTINGS_TYPE};
use be_block_keys::{Party, RecoveryPublic, RecoverySecret};
use be_block_protocol::{
    ErrorCode, KeyKind, OrganizationKind, OrganizationRole, ServerAccount, WrappedBy,
};
use be_block_store::{ContentKey, Keyring, Vault};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::ServerError;
use crate::keys::{encode_wrapped_by, keys_of_block, organization_kind, wraps_of};
use crate::store::{Identity, ServerStore, recovery_key_of};

const KEY_DEPTH: usize = 64;

#[derive(Clone)]
pub(crate) struct ServerKey {
    pub(crate) account: Uuid,
    secret: RecoverySecret,
}

impl ServerKey {
    pub(crate) fn load_or_create(connection: &Connection) -> Result<Self, ServerError> {
        if let Some(held) = Self::load(connection)? {
            return Ok(held);
        }
        let transaction = be_block_sqlite_schema::write_transaction(connection)?;
        let made = match Self::load(&transaction)? {
            Some(held) => held,
            None => Self::create(&transaction)?,
        };
        transaction.commit()?;
        Ok(made)
    }

    fn load(connection: &Connection) -> Result<Option<Self>, ServerError> {
        let held: Option<(String, Vec<u8>)> = connection
            .query_row(
                "SELECT account_id, secret FROM server_account WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        held.map(|(account, secret)| {
            Ok(Self {
                account: crate::store::parse_uuid(&account)?,
                secret: RecoverySecret::from_bytes(
                    secret.try_into().map_err(|_| ServerError::Corrupt)?,
                ),
            })
        })
        .transpose()
    }

    fn create(connection: &Connection) -> Result<Self, ServerError> {
        let account = Uuid::new_v4();
        let secret = RecoverySecret::from_bytes(crate::new_secret_bytes());
        connection.execute(
            "INSERT INTO accounts (id, email, display_name) VALUES (?1, ?2, ?3)",
            params![
                account.to_string(),
                format!("server-{}@server.invalid", account.simple()),
                "This server"
            ],
        )?;
        connection.execute(
            "INSERT INTO recovery_keys (account_id, public) VALUES (?1, ?2)",
            params![account.to_string(), secret.public().0.to_vec()],
        )?;
        connection.execute(
            "INSERT INTO server_account (id, account_id, secret) VALUES (1, ?1, ?2)",
            params![account.to_string(), secret.to_bytes().to_vec()],
        )?;
        Ok(Self { account, secret })
    }

    /// A secret for `purpose` derived from the server's own, so it lasts as long as the data
    /// directory without being stored apart from it.
    pub(crate) fn derived(&self, purpose: &str) -> [u8; 32] {
        let mut derived = [0u8; 32];
        let secret = zeroize::Zeroizing::new(self.secret.to_bytes());
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"be3 server derived"), secret.as_slice())
            .expand(purpose.as_bytes(), &mut derived)
            .expect("32 bytes is a valid HKDF-SHA256 length");
        derived
    }

    pub(crate) fn described(&self) -> ServerAccount {
        ServerAccount {
            account: self.account,
            public: self.secret.public().0,
        }
    }

    fn open_key(
        &self,
        connection: &Connection,
        organization: Uuid,
        key: Uuid,
        ring: &Keyring,
        depth: usize,
    ) -> Result<bool, ServerError> {
        if ring.contains(key) {
            return Ok(true);
        }
        if depth == 0 {
            return Ok(false);
        }
        for wrap in wraps_of(connection, organization, key)? {
            let plain = match wrap.wrapped_by {
                WrappedBy::Account(account) if account == self.account => {
                    let Some(public) = recovery_key_of(connection, wrap.sender)? else {
                        continue;
                    };
                    let sender = Party {
                        account: wrap.sender.into_bytes(),
                        public: RecoveryPublic(public),
                    };
                    self.secret
                        .open_from(
                            &sender,
                            self.account.into_bytes(),
                            key.as_bytes(),
                            &wrap.sealed,
                        )
                        .ok()
                }
                WrappedBy::Account(_) => None,
                WrappedBy::Key(by) => {
                    let escrow = crate::keys::key_row(connection, organization, by)?
                        .is_none_or(|row| row.kind == KeyKind::Escrow);
                    match !escrow && self.open_key(connection, organization, by, ring, depth - 1)? {
                        true => ring.open(&wrap.sealed).ok(),
                        false => None,
                    }
                }
            };
            // A wrap opens only to the key its row names: one the server's own table relabels
            // (as an older key served as the newest) opens nothing.
            let row = crate::keys::key_row(connection, organization, key)?;
            if let Some(carried) = plain
                .and_then(|plain| be_block_keys::CarriedKey::decode(&plain).ok())
                .filter(|carried| {
                    row.is_some_and(|row| {
                        carried.label == label(row) && carried.block == row.block.into_bytes()
                    })
                })
            {
                ring.insert(
                    key,
                    ContentKey::from_bytes(carried.key),
                    Uuid::from_bytes(carried.block),
                );
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn read<C: BlockContent>(
        &self,
        connection: &Connection,
        objects: &crate::objects::Objects,
        graph: &BlockGraph,
        organization: Uuid,
        block: Uuid,
    ) -> Result<Option<C>, ServerError> {
        let Some(head) = graph.head(block) else {
            return Ok(None);
        };
        let ring = Keyring::new();
        for row in keys_of_block(connection, organization, block)? {
            if row.kind == KeyKind::Data {
                self.open_key(connection, organization, row.id, &ring, KEY_DEPTH)?;
            }
        }
        let commits = CommitStore::new(Vault::new(objects.clone(), ring));
        let content = commits
            .get(head)
            .and_then(|commit| commits.vault().read(&commit.manifest))
            .map_err(|_| unreadable(block))?;
        C::decode(&content).map(Some).map_err(|_| unreadable(block))
    }

    /// Seals again, from the server itself, what `account` sealed to it by its `old` public key
    /// (the organization settings' keys, `Peer::share_with_server`), before a new phrase replaces
    /// that key: a wrap opens by its sender's current key (`open_key`), so the server would
    /// otherwise stop reading the policies. A wrap that does not open by `old` is left as it is.
    pub(crate) fn reseal_from(
        &self,
        connection: &Connection,
        account: Uuid,
        old: [u8; 32],
    ) -> Result<(), ServerError> {
        let sender = Party {
            account: account.into_bytes(),
            public: RecoveryPublic(old),
        };
        let me = Party {
            account: self.account.into_bytes(),
            public: self.secret.public(),
        };
        let by = encode_wrapped_by(WrappedBy::Account(self.account));
        let held: Vec<(String, String, Vec<u8>)> = connection
            .prepare_cached(
                "SELECT organization_id, key_id, sealed FROM wrapped_keys
                 WHERE wrapped_by = ?1 AND sender = ?2",
            )?
            .query_map(params![by, account.to_string()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        for (organization, key, sealed) in held {
            let id = crate::store::parse_uuid(&key)?;
            let Ok(plain) =
                self.secret
                    .open_from(&sender, self.account.into_bytes(), id.as_bytes(), &sealed)
            else {
                continue;
            };
            let sealed = self
                .secret
                .seal_for(self.account.into_bytes(), &me, id.as_bytes(), &plain)
                .map_err(|_| ServerError::Corrupt)?;
            connection.execute(
                "UPDATE wrapped_keys SET sender = ?4, sealed = ?5
                 WHERE organization_id = ?1 AND key_id = ?2 AND wrapped_by = ?3",
                params![organization, key, by, self.account.to_string(), sealed],
            )?;
        }
        Ok(())
    }

    pub(crate) fn seal_settings_to(
        &self,
        connection: &Connection,
        graph: &BlockGraph,
        organization: Uuid,
        account: Uuid,
    ) -> Result<(), ServerError> {
        let settings = settings_of(organization);
        // A guest reads no settings: it holds only what links granted it.
        if account == self.account
            || !graph.contains(settings)
            || crate::contacts::role_in(connection, account, organization)?
                == Some(be_block_protocol::OrganizationRole::Guest)
        {
            return Ok(());
        }
        let Some(public) = recovery_key_of(connection, account)? else {
            return Ok(());
        };
        let to = Party {
            account: account.into_bytes(),
            public: RecoveryPublic(public),
        };
        let by = encode_wrapped_by(WrappedBy::Account(account));
        for row in keys_of_block(connection, organization, settings)? {
            if row.kind != KeyKind::Scope
                || wraps_of(connection, organization, row.id)?
                    .iter()
                    .any(|wrap| wrap.wrapped_by == WrappedBy::Account(account))
            {
                continue;
            }
            let ring = Keyring::new();
            if !self.open_key(connection, organization, row.id, &ring, KEY_DEPTH)? {
                continue;
            }
            let plain = ring.get(row.id).ok_or(ServerError::Corrupt)?;
            // The server vouches for no root (decisions/the_server_never_vouches_for_an_identity.md)
            // nor head, so it carries none: its seal is never sealed on by a log.
            let carried = be_block_keys::CarriedKey {
                key: *plain.as_bytes(),
                label: label(row),
                block: settings.into_bytes(),
                root: None,
                heads: be_block_keys::Heads::default(),
            };
            let sealed = self.secret.seal_for(
                self.account.into_bytes(),
                &to,
                row.id.as_bytes(),
                &carried.encode(),
            );
            // An account whose key nothing can be sealed to is sealed nothing.
            let Ok(sealed) = sealed else {
                continue;
            };
            connection.execute(
                "INSERT OR REPLACE INTO wrapped_keys
                    (organization_id, key_id, wrapped_by, sender, sealed)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    organization.to_string(),
                    row.id.to_string(),
                    by,
                    self.account.to_string(),
                    sealed
                ],
            )?;
        }
        Ok(())
    }
}

fn unreadable(block: Uuid) -> ServerError {
    ServerError::Refused(
        ErrorCode::PolicyUnreadable,
        format!(
            "the server cannot read this organization's policies in block {block}, so it refuses what they govern until an administrator shares them with the server again"
        ),
    )
}

pub(crate) fn settings_of(organization: Uuid) -> Uuid {
    be_block::organization_settings(organization)
}

pub(crate) fn in_settings(graph: &BlockGraph, organization: Uuid, block: Uuid) -> bool {
    let settings = settings_of(organization);
    graph.contains(settings)
        && (block == settings || graph.ancestors(block).any(|up| up == settings))
}

fn account_exists(connection: &Connection, account: Uuid) -> Result<bool, ServerError> {
    Ok(connection
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)")?
        .query_row([account.to_string()], |row| row.get(0))?)
}

pub(crate) fn server_account(connection: &Connection) -> Result<Uuid, ServerError> {
    let account: String = connection
        .prepare_cached("SELECT account_id FROM server_account WHERE id = 1")?
        .query_row([], |row| row.get(0))?;
    crate::store::parse_uuid(&account)
}

#[derive(Clone)]
pub(crate) struct HeldPolicies {
    settings: Option<CommitId>,
    policies: Option<(Uuid, Option<CommitId>)>,
    value: OrganizationPolicies,
}

impl ServerStore {
    pub(crate) fn policies(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        organization: Uuid,
    ) -> Result<OrganizationPolicies, ServerError> {
        if organization_kind(connection, organization)? != OrganizationKind::Company {
            return Ok(OrganizationPolicies::default());
        }
        let settings = settings_of(organization);
        if !graph.contains(settings) {
            return Ok(OrganizationPolicies::default());
        }
        let head = graph.head(settings);
        let held = self.held_policies(organization);
        let block = match &held {
            Some(held) if held.settings == head => held.policies.map(|(block, _)| block),
            _ => self
                .server_key()
                .read::<SettingsContent>(connection, self.objects(), graph, organization, settings)?
                .and_then(|content| content.root().policy(OrganizationPolicies::CONTENT_TYPE))
                .filter(|block| *block != settings && in_settings(graph, organization, *block)),
        };
        let policies = block.map(|block| (block, graph.head(block)));
        if let Some(held) = &held
            && held.settings == head
            && held.policies == policies
        {
            return Ok(held.value.clone());
        }
        let value = match block {
            Some(block) => self
                .server_key()
                .read::<OrganizationPoliciesContent>(
                    connection,
                    self.objects(),
                    graph,
                    organization,
                    block,
                )?
                .map(|content| content.root())
                .unwrap_or_default(),
            None => OrganizationPolicies::default(),
        };
        self.hold_policies(
            organization,
            HeldPolicies {
                settings: head,
                policies,
                value: value.clone(),
            },
        );
        Ok(value)
    }

    fn held_policies(&self, organization: Uuid) -> Option<HeldPolicies> {
        self.policy_cache()
            .lock()
            .expect("the policy cache is never poisoned")
            .get(&organization)
            .cloned()
    }

    fn hold_policies(&self, organization: Uuid, held: HeldPolicies) {
        self.policy_cache()
            .lock()
            .expect("the policy cache is never poisoned")
            .insert(organization, held);
    }
}

pub(crate) fn refused(message: impl Into<String>) -> ServerError {
    ServerError::Refused(ErrorCode::PolicyRefused, message.into())
}

impl ServerStore {
    pub(crate) fn may_create(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        identity: Identity,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
    ) -> Result<(), ServerError> {
        if graph.contains(block) {
            return Ok(());
        }
        let administrator = identity.role == OrganizationRole::Administrator;
        if block == settings_of(identity.organization) {
            let company =
                organization_kind(connection, identity.organization)? == OrganizationKind::Company;
            if !company
                || !administrator
                || content_type != SETTINGS_TYPE
                || parent != BlockParent::Root
            {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "only an administrator makes a company organization's settings, as settings at the top of it".into(),
                ));
            }
            return Ok(());
        }
        let claimed = be_block::my_files_of(identity.organization, block);
        if claimed != identity.account && account_exists(connection, claimed)? {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "this id is kept for another account's My files".into(),
            ));
        }
        let claimed = be_block::unfiled_of(identity.organization, block);
        if claimed != identity.account && account_exists(connection, claimed)? {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "this id is kept for another account's Unfiled".into(),
            ));
        }
        let claimed = be_block::account_block_of(identity.organization, block);
        if claimed != identity.account && account_exists(connection, claimed)? {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "this id is kept for another account's own block".into(),
            ));
        }
        if content_type == <be_block::AccountBlock as be_block_content::Root>::CONTENT_TYPE
            && block != be_block::account_block(identity.organization, identity.account)
        {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "an account's own block is made only at its own id".into(),
            ));
        }
        let own_unfiled = block == be_block::unfiled(identity.organization, identity.account);
        if own_unfiled != (content_type == be_block::UNFILED_TYPE)
            || (own_unfiled && parent != BlockParent::Root)
        {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "an account's Unfiled is made only at its own id, at the top of the organization"
                    .into(),
            ));
        }
        if parent != BlockParent::Root {
            return Ok(());
        }
        self.may_add_workspace(graph, connection, identity, block, content_type)
    }

    pub(crate) fn may_add_workspace(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        identity: Identity,
        block: Uuid,
        content_type: Uuid,
    ) -> Result<(), ServerError> {
        if identity.role == OrganizationRole::Administrator
            || !be_block::is_workspace_type(content_type)
            || (block == be_block::my_files(identity.organization, identity.account)
                && content_type == WORKSPACE_TYPE)
        {
            return Ok(());
        }
        match self
            .policies(graph, connection, identity.organization)?
            .workspace_creation()
        {
            WhoMay::Administrators => Err(refused(
                "only administrators make workspaces in this organization; each member has My files",
            )),
            WhoMay::Everyone => Ok(()),
        }
    }
}

/// The label a key carries of its own row (`be_block_keys::Label`).
fn label(row: be_block_protocol::KeyRow) -> be_block_keys::Label {
    be_block_keys::Label {
        id: row.id.into_bytes(),
        kind: match row.kind {
            KeyKind::Data => be_block_keys::KeyRole::Data,
            KeyKind::Scope => be_block_keys::KeyRole::Scope,
            KeyKind::Escrow => be_block_keys::KeyRole::Escrow,
        },
        epoch: row.epoch,
    }
}
