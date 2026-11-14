//! What the server keeps for push notifications: each account's endpoints per organization and
//! its mode per block, and who a notification goes to. guides/blocks/server.md, "Notifications".

use std::collections::{BTreeSet, HashMap};

use be_block_protocol::{BlockNotifyMode, ErrorCode, NotifyMode, PushEndpoint, PushService};
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::{Identity, ServerError, limits::MAX_PUSH_ENDPOINTS, store::ServerStore};

/// A member a notification goes to, with the endpoints of their devices, and whether it
/// mentions them.
pub(crate) struct Recipient {
    pub(crate) account: Uuid,
    /// Each endpoint, and whether it has failures counted.
    pub(crate) endpoints: Vec<(PushEndpoint, bool)>,
    pub(crate) mentioned: bool,
}

fn service_name(service: PushService) -> &'static str {
    match service {
        PushService::WebPush => "web-push",
        PushService::Fcm => "fcm",
    }
}

fn service_of(name: &str) -> Result<PushService, ServerError> {
    match name {
        "web-push" => Ok(PushService::WebPush),
        "fcm" => Ok(PushService::Fcm),
        _ => Err(ServerError::Corrupt),
    }
}

fn mode_name(mode: NotifyMode) -> &'static str {
    match mode {
        NotifyMode::All => "all",
        NotifyMode::Mentions => "mentions",
        NotifyMode::Off => "off",
    }
}

fn mode_of(name: &str) -> Result<NotifyMode, ServerError> {
    match name {
        "all" => Ok(NotifyMode::All),
        "mentions" => Ok(NotifyMode::Mentions),
        "off" => Ok(NotifyMode::Off),
        _ => Err(ServerError::Corrupt),
    }
}

/// Forgets a deleted block's modes and what was told about it.
pub(crate) fn forget(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<(), ServerError> {
    let (organization, block) = (organization.to_string(), block.to_string());
    database.execute(
        "DELETE FROM notify_modes WHERE organization_id = ?1 AND block_id = ?2",
        params![organization, block],
    )?;
    database.execute(
        "DELETE FROM told_keys WHERE organization_id = ?1 AND block_id = ?2",
        params![organization, block],
    )?;
    Ok(())
}

/// Forgets what a member removed from `organization` kept there: their endpoints, modes and
/// what they told.
pub(crate) fn forget_member(
    database: &Connection,
    organization: Uuid,
    account: Uuid,
) -> Result<(), ServerError> {
    let (organization, account) = (organization.to_string(), account.to_string());
    database.execute(
        "DELETE FROM push_endpoints WHERE organization_id = ?1 AND account_id = ?2",
        params![organization, account],
    )?;
    database.execute(
        "DELETE FROM notify_modes WHERE organization_id = ?1 AND account_id = ?2",
        params![organization, account],
    )?;
    database.execute(
        "DELETE FROM told_keys WHERE organization_id = ?1 AND account_id = ?2",
        params![organization, account],
    )?;
    Ok(())
}

fn milliseconds() -> i64 {
    i64::try_from(crate::push::unix_now()).unwrap_or(i64::MAX / 1000) * 1000
}

fn not_viewable() -> ServerError {
    ServerError::Refused(ErrorCode::BlockNotFound, "block not found".into())
}

impl ServerStore {
    /// Notes `keys` told about `block` from `identity`'s account, and answers whether any was
    /// not told before: a device that sent a notification and lost the answer sends it again,
    /// and the members hear it at most once (the keys are kept before the push is queued, so a
    /// push that then fails is not sent again). The newest `MAX_TOLD_KEYS` are remembered per
    /// account and block; a key older than those is news again.
    pub(crate) async fn tell(
        &self,
        identity: Identity,
        block: Uuid,
        keys: Vec<be_block_store::Hash>,
    ) -> Result<bool, ServerError> {
        self.with_database(move |_, database| {
            let (organization, block, account) = (
                identity.organization.to_string(),
                block.to_string(),
                identity.account.to_string(),
            );
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let mut new = false;
            for key in &keys {
                new |= transaction.execute(
                    "INSERT OR IGNORE INTO told_keys (organization_id, block_id, account_id, key)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![organization, block, account, key.as_bytes().as_slice()],
                )? > 0;
            }
            transaction.execute(
                "DELETE FROM told_keys
                 WHERE organization_id = ?1 AND block_id = ?2 AND account_id = ?3
                 AND rowid NOT IN (
                     SELECT rowid FROM told_keys
                     WHERE organization_id = ?1 AND block_id = ?2 AND account_id = ?3
                     ORDER BY rowid DESC LIMIT ?4
                 )",
                params![
                    organization,
                    block,
                    account,
                    be_block_protocol::MAX_TOLD_KEYS as i64
                ],
            )?;
            transaction.commit()?;
            Ok(new)
        })
        .await
    }

    /// Registers `endpoint` for `identity`'s account in its organization, replacing one at the
    /// same address and dropping the oldest past `MAX_PUSH_ENDPOINTS`.
    pub(crate) async fn register_endpoint(
        &self,
        identity: Identity,
        endpoint: PushEndpoint,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            let (account, organization) = (
                identity.account.to_string(),
                identity.organization.to_string(),
            );
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            transaction.execute(
                "DELETE FROM push_endpoints
                 WHERE account_id = ?1 AND organization_id = ?2 AND address = ?3",
                params![account, organization, endpoint.address],
            )?;
            transaction.execute(
                "INSERT INTO push_endpoints
                 (account_id, organization_id, address, service, p256dh, auth)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    account,
                    organization,
                    endpoint.address,
                    service_name(endpoint.service),
                    endpoint.p256dh,
                    endpoint.auth,
                ],
            )?;
            transaction.execute(
                "DELETE FROM push_endpoints
                 WHERE account_id = ?1 AND organization_id = ?2 AND rowid NOT IN (
                     SELECT rowid FROM push_endpoints
                     WHERE account_id = ?1 AND organization_id = ?2
                     ORDER BY rowid DESC LIMIT ?3
                 )",
                params![account, organization, MAX_PUSH_ENDPOINTS],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn unregister_endpoint(
        &self,
        identity: Identity,
        address: String,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            database.execute(
                "DELETE FROM push_endpoints
                 WHERE account_id = ?1 AND organization_id = ?2 AND address = ?3",
                params![
                    identity.account.to_string(),
                    identity.organization.to_string(),
                    address
                ],
            )?;
            Ok(())
        })
        .await
    }

    /// Forgets `account`'s endpoint at `address` in every organization: the device signed out.
    pub(crate) async fn forget_endpoint_of(
        &self,
        account: Uuid,
        address: String,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            database.execute(
                "DELETE FROM push_endpoints WHERE account_id = ?1 AND address = ?2",
                params![account.to_string(), address],
            )?;
            Ok(())
        })
        .await
    }

    /// Drops an endpoint its push service said is gone, for every account that registered it.
    pub(crate) async fn drop_endpoint(&self, address: String) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            database.execute(
                "DELETE FROM push_endpoints WHERE address = ?1",
                params![address],
            )?;
            Ok(())
        })
        .await
    }

    /// Notes that an endpoint's service took a push: it is failing no longer.
    pub(crate) async fn endpoint_answered(
        &self,
        account: Uuid,
        organization: Uuid,
        address: String,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            database.execute(
                "UPDATE push_endpoints SET failures = 0, failing_since = NULL
                 WHERE account_id = ?1 AND organization_id = ?2 AND address = ?3",
                params![account.to_string(), organization.to_string(), address],
            )?;
            Ok(())
        })
        .await
    }

    /// Counts a failed push against an endpoint, and drops it once it has failed
    /// `push::MAX_FAILURES` times in a row over at least `push::FAILING_FOR`.
    pub(crate) async fn endpoint_failed(
        &self,
        account: Uuid,
        organization: Uuid,
        address: String,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            let now = milliseconds();
            let (account, organization) = (account.to_string(), organization.to_string());
            database.execute(
                "UPDATE push_endpoints
                 SET failures = failures + 1, failing_since = COALESCE(failing_since, ?4)
                 WHERE account_id = ?1 AND organization_id = ?2 AND address = ?3",
                params![account, organization, address, now],
            )?;
            let since =
                now - i64::try_from(crate::push::FAILING_FOR.as_millis()).unwrap_or(i64::MAX);
            database.execute(
                "DELETE FROM push_endpoints
                 WHERE account_id = ?1 AND organization_id = ?2 AND address = ?3
                 AND failures >= ?4 AND failing_since <= ?5",
                params![
                    account,
                    organization,
                    address,
                    crate::push::MAX_FAILURES,
                    since
                ],
            )?;
            Ok(())
        })
        .await
    }

    /// The endpoints `account` registered in `organization`, oldest first.
    pub async fn push_endpoints(
        &self,
        account: Uuid,
        organization: Uuid,
    ) -> Result<Vec<PushEndpoint>, ServerError> {
        self.with_database(move |_, database| endpoints_of(database, account, organization))
            .await
    }

    /// Every block in `identity`'s organization whose mode its account set to other than `all`.
    pub(crate) async fn notify_modes(
        &self,
        identity: Identity,
    ) -> Result<Vec<BlockNotifyMode>, ServerError> {
        self.with_database(move |_, database| {
            let mut statement = database.prepare_cached(
                "SELECT block_id, mode FROM notify_modes
                 WHERE organization_id = ?1 AND account_id = ?2",
            )?;
            let rows = statement.query_map(
                params![
                    identity.organization.to_string(),
                    identity.account.to_string()
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            let mut modes = Vec::new();
            for row in rows {
                let (block, mode) = row?;
                modes.push(BlockNotifyMode {
                    block: crate::store::parse_uuid(&block)?,
                    mode: mode_of(&mode)?,
                });
            }
            Ok(modes)
        })
        .await
    }

    /// Sets how `identity`'s account is notified about `block`; `all`, the default, keeps no
    /// row.
    pub(crate) async fn set_notify_mode(
        &self,
        identity: Identity,
        block: Uuid,
        mode: NotifyMode,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            let (organization, block, account) = (
                identity.organization.to_string(),
                block.to_string(),
                identity.account.to_string(),
            );
            match mode {
                NotifyMode::All => database.execute(
                    "DELETE FROM notify_modes
                     WHERE organization_id = ?1 AND block_id = ?2 AND account_id = ?3",
                    params![organization, block, account],
                )?,
                mode => database.execute(
                    "INSERT INTO notify_modes (organization_id, block_id, account_id, mode)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (organization_id, block_id, account_id)
                     DO UPDATE SET mode = excluded.mode",
                    params![organization, block, account, mode_name(mode)],
                )?,
            };
            Ok(())
        })
        .await
    }

    /// Who a notification about `block` from `identity` goes to, with their endpoints: every
    /// account that may view the block, but the sender, whose mode lets it through (`off`
    /// never, `mentions` only when `mentions` names it) and who registered an endpoint here.
    /// Whether each is connected is the caller's to judge.
    pub(crate) async fn notification_recipients(
        &self,
        identity: Identity,
        block: Uuid,
        mentions: Vec<Uuid>,
    ) -> Result<Vec<Recipient>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            if !graph.contains(block) {
                return Err(not_viewable());
            }
            let mut viewers: BTreeSet<Uuid> = graph.viewers(block);
            viewers.remove(&identity.account);
            let modes = modes_of(database, identity.organization, block)?;
            let mentioned: BTreeSet<Uuid> = mentions.into_iter().collect();
            let mut wanted: HashMap<Uuid, bool> = HashMap::new();
            for account in viewers {
                let mentions = mentioned.contains(&account);
                let wants = match modes.get(&account).copied().unwrap_or(NotifyMode::All) {
                    NotifyMode::All => true,
                    NotifyMode::Mentions => mentions,
                    NotifyMode::Off => false,
                };
                if wants {
                    wanted.insert(account, mentions);
                }
            }
            let mut recipients: Vec<Recipient> = Vec::new();
            let mut index: HashMap<Uuid, usize> = HashMap::new();
            for (account, endpoint, failing) in
                organization_endpoints(database, identity.organization)?
            {
                let Some(mentioned) = wanted.get(&account) else {
                    continue;
                };
                let at = *index.entry(account).or_insert_with(|| {
                    recipients.push(Recipient {
                        account,
                        endpoints: Vec::new(),
                        mentioned: *mentioned,
                    });
                    recipients.len() - 1
                });
                recipients[at].endpoints.push((endpoint, failing));
            }
            Ok(recipients)
        })
        .await
    }
}

fn modes_of(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<HashMap<Uuid, NotifyMode>, ServerError> {
    let mut statement = database.prepare_cached(
        "SELECT account_id, mode FROM notify_modes WHERE organization_id = ?1 AND block_id = ?2",
    )?;
    let rows = statement.query_map(
        params![organization.to_string(), block.to_string()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    let mut modes = HashMap::new();
    for row in rows {
        let (account, mode) = row?;
        modes.insert(crate::store::parse_uuid(&account)?, mode_of(&mode)?);
    }
    Ok(modes)
}

/// Every endpoint registered in `organization`, with its account, oldest first: one query for a
/// notification however many may view its block.
fn organization_endpoints(
    database: &Connection,
    organization: Uuid,
) -> Result<Vec<(Uuid, PushEndpoint, bool)>, ServerError> {
    let mut statement = database.prepare_cached(
        "SELECT account_id, service, address, p256dh, auth, failures FROM push_endpoints
         WHERE organization_id = ?1 ORDER BY rowid",
    )?;
    let rows = statement.query_map(params![organization.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, Vec<u8>>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;
    let mut endpoints = Vec::new();
    for row in rows {
        let (account, service, address, p256dh, auth, failures) = row?;
        endpoints.push((
            crate::store::parse_uuid(&account)?,
            PushEndpoint {
                service: service_of(&service)?,
                address,
                p256dh,
                auth,
            },
            failures > 0,
        ));
    }
    Ok(endpoints)
}

fn endpoints_of(
    database: &Connection,
    account: Uuid,
    organization: Uuid,
) -> Result<Vec<PushEndpoint>, ServerError> {
    let mut statement = database.prepare_cached(
        "SELECT service, address, p256dh, auth FROM push_endpoints
         WHERE account_id = ?1 AND organization_id = ?2 ORDER BY rowid",
    )?;
    let rows = statement.query_map(
        params![account.to_string(), organization.to_string()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        },
    )?;
    let mut endpoints = Vec::new();
    for row in rows {
        let (service, address, p256dh, auth) = row?;
        endpoints.push(PushEndpoint {
            service: service_of(&service)?,
            address,
            p256dh,
            auth,
        });
    }
    Ok(endpoints)
}
