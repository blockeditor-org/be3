//! What the server keeps for push notifications: each account's endpoints per organization and
//! its mode per block, and who a notification goes to. guides/blocks/server.md, "Notifications".

use std::collections::{BTreeSet, HashMap};

use be_block_protocol::{ErrorCode, NotifyMode, PushEndpoint, PushService};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{Identity, ServerError, limits::MAX_PUSH_ENDPOINTS, store::ServerStore};

/// A member a notification goes to, with the endpoints of their devices.
pub(crate) struct Recipient {
    pub(crate) account: Uuid,
    pub(crate) endpoints: Vec<PushEndpoint>,
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

/// Forgets a deleted block's modes.
pub(crate) fn forget(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<(), ServerError> {
    database.execute(
        "DELETE FROM notify_modes WHERE organization_id = ?1 AND block_id = ?2",
        params![organization.to_string(), block.to_string()],
    )?;
    Ok(())
}

fn not_viewable() -> ServerError {
    ServerError::Refused(ErrorCode::BlockNotFound, "block not found".into())
}

impl ServerStore {
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
            let transaction = database.unchecked_transaction()?;
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

    /// The endpoints `account` registered in `organization`, oldest first.
    pub async fn push_endpoints(
        &self,
        account: Uuid,
        organization: Uuid,
    ) -> Result<Vec<PushEndpoint>, ServerError> {
        self.with_database(move |_, database| {
            endpoints_of(database, account, organization)
        })
        .await
    }

    pub(crate) async fn notify_mode(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<NotifyMode, ServerError> {
        self.with_database(move |_, database| {
            let mode: Option<String> = database
                .query_row(
                    "SELECT mode FROM notify_modes
                     WHERE organization_id = ?1 AND block_id = ?2 AND account_id = ?3",
                    params![
                        identity.organization.to_string(),
                        block.to_string(),
                        identity.account.to_string()
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            mode.map_or(Ok(NotifyMode::All), |mode| mode_of(&mode))
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
            let mut recipients = Vec::new();
            for account in viewers {
                let wanted = match modes.get(&account).copied().unwrap_or(NotifyMode::All) {
                    NotifyMode::All => true,
                    NotifyMode::Mentions => mentioned.contains(&account),
                    NotifyMode::Off => false,
                };
                if !wanted {
                    continue;
                }
                let endpoints = endpoints_of(database, account, identity.organization)?;
                if !endpoints.is_empty() {
                    recipients.push(Recipient { account, endpoints });
                }
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
