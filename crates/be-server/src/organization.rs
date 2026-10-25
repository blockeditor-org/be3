use std::collections::BTreeSet;

use be_block::{
    BlockContent, OrganizationPolicies, OrganizationPoliciesContent, Root, SettingsContent,
    WORKSPACE_TYPE, WhoMay,
};
use be_commit::{CommitId, CommitStore};
use be_graph::{BlockGraph, BlockParent, SETTINGS_TYPE};
use be_keys::{Party, RecoveryPublic, RecoverySecret};
use be_protocol::{
    ErrorCode, KeyKind, OrganizationKind, OrganizationRole, ServerAccount, WrappedBy,
};
use be_store::{ContentKey, FileStore, Hash, Keyring, Vault};
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
        connection.execute_batch("BEGIN IMMEDIATE")?;
        let made = Self::load(connection).and_then(|held| match held {
            Some(held) => Ok(held),
            None => Self::create(connection),
        });
        match made {
            Ok(made) => {
                connection.execute_batch("COMMIT")?;
                Ok(made)
            }
            Err(error) => {
                let _ = connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
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
        let secret = RecoverySecret::from_bytes(random_secret());
        connection.execute(
            "INSERT INTO accounts (id, email, display_name, password_hash) VALUES (?1, ?2, ?3, '!')",
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
            if let Some(bytes) = plain.and_then(|plain| <[u8; 32]>::try_from(plain).ok()) {
                ring.insert(key, ContentKey::from_bytes(bytes));
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn read<C: BlockContent>(
        &self,
        connection: &Connection,
        objects: &FileStore,
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

    pub(crate) fn seal_settings_to(
        &self,
        connection: &Connection,
        graph: &BlockGraph,
        organization: Uuid,
        account: Uuid,
    ) -> Result<(), ServerError> {
        let settings = settings_of(organization);
        if account == self.account || !graph.contains(settings) {
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
            let sealed = self.secret.seal_for(
                self.account.into_bytes(),
                &to,
                row.id.as_bytes(),
                plain.as_bytes(),
            );
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

fn random_secret() -> [u8; 32] {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes
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
        if content_type != WORKSPACE_TYPE || parent != BlockParent::Root {
            return Ok(());
        }
        self.may_add_workspace(graph, connection, identity)
    }

    pub(crate) fn may_add_workspace(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        identity: Identity,
    ) -> Result<(), ServerError> {
        if identity.role == OrganizationRole::Administrator {
            return Ok(());
        }
        if self
            .policies(graph, connection, identity.organization)?
            .workspace_creation()
            != WhoMay::Administrators
        {
            return Ok(());
        }
        let has_one = graph.children(BlockParent::Root).into_iter().any(|root| {
            graph.get(root).is_some_and(|node| {
                node.content_type == WORKSPACE_TYPE && node.author == identity.account
            })
        });
        match has_one {
            true => Err(refused(
                "only administrators make workspaces in this organization; each member keeps one of their own",
            )),
            false => Ok(()),
        }
    }

    pub async fn may_store(
        &self,
        identity: Identity,
        hashes: Vec<Hash>,
    ) -> Result<(), ServerError> {
        self.with_graph(identity.organization, move |graph, connection| {
            self.within_quota(graph, connection, identity.organization, &hashes)
        })
        .await
    }

    pub(crate) fn within_quota(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        organization: Uuid,
        hashes: &[Hash],
    ) -> Result<(), ServerError> {
        let Some(quota) = self.policies(graph, connection, organization)?.quota() else {
            return Ok(());
        };
        let organization = organization.to_string();
        let mut fresh = BTreeSet::new();
        for hash in hashes {
            let hex = hash.to_hex();
            let known: bool = connection
                .prepare_cached(
                    "SELECT EXISTS(SELECT 1 FROM commit_objects WHERE organization_id = ?1 AND hash = ?2)
                     OR EXISTS(SELECT 1 FROM block_objects WHERE organization_id = ?1 AND hash = ?2)",
                )?
                .query_row(params![organization, hex], |row| row.get(0))?;
            if !known {
                fresh.insert(hex);
            }
        }
        if fresh.is_empty() {
            return Ok(());
        }
        let held: i64 = connection.query_row(
            "SELECT COUNT(*) FROM (
                SELECT hash FROM commit_objects WHERE organization_id = ?1
                UNION SELECT hash FROM block_objects WHERE organization_id = ?1)",
            [&organization],
            |row| row.get(0),
        )?;
        let total = u64::try_from(held).unwrap_or(0) + fresh.len() as u64;
        match total <= quota {
            true => Ok(()),
            false => Err(ServerError::Refused(
                ErrorCode::OverQuota,
                format!(
                    "this organization stores at most {quota} objects, and this would make {total}"
                ),
            )),
        }
    }
}
