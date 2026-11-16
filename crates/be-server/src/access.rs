//! Access logs (plans/signed_access.md): each block's signed records, root first, kept in the
//! clear and checked here as devices check them, so a good server and a device never disagree.

use be_block_keys::{
    Authority, Checked, Entry, EntryHash, Level, LogKind, Logs, PublicIdentity, Record,
    RecoveryPublic, SigningPublic,
};
use be_block_graph::{Access, BlockGraph};
use be_block_protocol::ErrorCode;
use be_block_protocol::{Retirement, SignedLog};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::ServerError;

fn refused(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

/// `account`'s keys as the server holds them.
fn identity_of(
    connection: &Connection,
    account: Uuid,
) -> Result<Option<PublicIdentity>, ServerError> {
    let row = connection
        .prepare_cached("SELECT public, signing FROM recovery_keys WHERE account_id = ?1")?
        .query_row([account.to_string()], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .optional()?;
    let Some((public, signing)) = row else {
        return Ok(None);
    };
    let (Ok(public), Ok(signing)) = (<[u8; 32]>::try_from(public), <[u8; 32]>::try_from(signing))
    else {
        return Ok(None);
    };
    Ok(Some(PublicIdentity {
        public: RecoveryPublic(public),
        signing: SigningPublic(signing),
    }))
}

/// `root` as the first entry of the log of `log`, once it checks: well formed, made by `account`
/// with the keys the server holds for it now.
fn checked_root(
    connection: &Connection,
    account: Uuid,
    kind: LogKind,
    log: Uuid,
    root: &[u8],
) -> Result<Entry, ServerError> {
    let entry = Entry::decode(root).map_err(|_| refused("the root does not read"))?;
    let checked = entry
        .checked_root(kind, log.into_bytes())
        .map_err(|_| refused("the root is not signed by whom it names"))?;
    if checked.creator != account.into_bytes() {
        return Err(refused("a root names the account that makes it"));
    }
    if identity_of(connection, account)? != Some(checked.identity) {
        return Err(refused("a root names the keys its creator holds now"));
    }
    Ok(entry)
}

/// Stores `root` as the first entry of a new block's log, once it checks (`checked_root`).
pub(crate) fn store_root(
    connection: &Connection,
    organization: Uuid,
    account: Uuid,
    block: Uuid,
    root: &[u8],
) -> Result<(), ServerError> {
    let entry = checked_root(connection, account, LogKind::Block, block, root)?;
    insert_entry(connection, LogKind::Block, organization, block, 0, &entry)?;
    Ok(())
}

/// The block's log, root first.
pub(crate) fn log_of(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Vec<Vec<u8>>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT entry FROM access_log WHERE organization_id = ?1 AND block_id = ?2
         ORDER BY position",
    )?;
    let entries = statement
        .query_map(
            params![organization.to_string(), block.to_string()],
            |row| row.get::<_, Vec<u8>>(0),
        )?
        .collect::<Result<_, _>>()?;
    Ok(entries)
}

/// Stores `root` as the first entry of a new company organization's log, once it checks
/// (`checked_root`).
pub(crate) fn store_organization_root(
    connection: &Connection,
    account: Uuid,
    organization: Uuid,
    root: &[u8],
) -> Result<(), ServerError> {
    let entry = checked_root(
        connection,
        account,
        LogKind::Organization,
        organization,
        root,
    )?;
    insert_entry(connection, LogKind::Organization, organization, organization, 0, &entry)?;
    Ok(())
}

/// The organization's log, root first: empty for the personal organization.
pub(crate) fn organization_log(
    connection: &Connection,
    organization: Uuid,
) -> Result<Vec<Vec<u8>>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT entry FROM organization_log WHERE organization_id = ?1 ORDER BY position",
    )?;
    let entries = statement
        .query_map(params![organization.to_string()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .collect::<Result<_, _>>()?;
    Ok(entries)
}

fn wire_kind(kind: LogKind) -> be_block_protocol::LogKind {
    match kind {
        LogKind::Block => be_block_protocol::LogKind::Block,
        LogKind::Organization => be_block_protocol::LogKind::Organization,
    }
}

fn log_kind(kind: be_block_protocol::LogKind) -> LogKind {
    match kind {
        be_block_protocol::LogKind::Block => LogKind::Block,
        be_block_protocol::LogKind::Organization => LogKind::Organization,
    }
}

/// The entries of a log, root first.
fn entries(
    connection: &Connection,
    kind: LogKind,
    organization: Uuid,
    log: Uuid,
) -> Result<Vec<Entry>, ServerError> {
    let encoded = match kind {
        LogKind::Block => log_of(connection, organization, log)?,
        LogKind::Organization => organization_log(connection, organization)?,
    };
    encoded
        .iter()
        .map(|entry| Entry::decode(entry).map_err(|_| ServerError::Corrupt))
        .collect()
}

/// Every log that names keys for `account` (`insert_entry`), at its head: those a new recovery
/// key retires the old ones in.
pub(crate) fn signed_logs(
    connection: &Connection,
    account: Uuid,
) -> Result<Vec<SignedLog>, ServerError> {
    let mut found = Vec::new();
    let mut statement = connection.prepare_cached(
        "SELECT organization_id, block_id FROM access_log_names WHERE account_id = ?1",
    )?;
    let blocks: Vec<(String, String)> = statement
        .query_map([account.to_string()], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut statement = connection.prepare_cached(
        "SELECT organization_id, organization_id FROM organization_log_names WHERE account_id = ?1",
    )?;
    let organizations: Vec<(String, String)> = statement
        .query_map([account.to_string()], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (kind, rows) in [
        (LogKind::Block, blocks),
        (LogKind::Organization, organizations),
    ] {
        for (organization, log) in rows {
            let organization = crate::store::parse_uuid(&organization)?;
            let log = crate::store::parse_uuid(&log)?;
            let head = entries(connection, kind, organization, log)?
                .last()
                .map(Entry::hash)
                .ok_or(ServerError::Corrupt)?;
            found.push(SignedLog {
                organization,
                kind: wire_kind(kind),
                log,
                head: head.to_vec(),
            });
        }
    }
    Ok(found)
}

/// Stores `entry` at `position` of its log, and the accounts it names keys for: its signer, and
/// the grantee, administrator or creator it names. Each is one a new recovery key must retire
/// in this log (`signed_logs`), or their old key would keep signing there.
fn insert_entry(
    connection: &Connection,
    kind: LogKind,
    organization: Uuid,
    log: Uuid,
    position: usize,
    entry: &Entry,
) -> Result<(), ServerError> {
    let named = match &entry.record {
        Record::Root(root) => root.creator,
        Record::Grant(grant) => grant.grantee,
        Record::Administrator(administrator) => administrator.account,
        Record::Retirement(retirement) => retirement.account,
    };
    let accounts = [entry.signer, named];
    match kind {
        LogKind::Block => {
            connection.execute(
                "INSERT INTO access_log (organization_id, block_id, position, entry)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    organization.to_string(),
                    log.to_string(),
                    position,
                    entry.encode()
                ],
            )?;
            for account in accounts {
                connection.execute(
                    "INSERT OR IGNORE INTO access_log_names (organization_id, block_id, account_id)
                     VALUES (?1, ?2, ?3)",
                    params![
                        organization.to_string(),
                        log.to_string(),
                        Uuid::from_bytes(account).to_string()
                    ],
                )?;
            }
        }
        LogKind::Organization => {
            connection.execute(
                "INSERT INTO organization_log (organization_id, position, entry)
                 VALUES (?1, ?2, ?3)",
                params![organization.to_string(), position, entry.encode()],
            )?;
            for account in accounts {
                connection.execute(
                    "INSERT OR IGNORE INTO organization_log_names (organization_id, account_id)
                     VALUES (?1, ?2)",
                    params![organization.to_string(), Uuid::from_bytes(account).to_string()],
                )?;
            }
        }
    }
    Ok(())
}

/// Appends `retirements`, which a new recovery key brings: exactly one for each log `account`
/// signed in, each the next entry of its log as devices check it, retiring the account's keys
/// for `new`.
pub(crate) fn append_retirements(
    connection: &Connection,
    account: Uuid,
    new: &PublicIdentity,
    retirements: &[Retirement],
) -> Result<(), ServerError> {
    let needed = signed_logs(connection, account)?;
    let mut given = std::collections::HashMap::new();
    for retirement in retirements {
        let at = (retirement.organization, retirement.kind, retirement.log);
        if given.insert(at, retirement).is_some()
            || !needed
                .iter()
                .any(|log| (log.organization, log.kind, log.log) == at)
        {
            return Err(refused(
                "a retirement goes in each log the account signed in, once",
            ));
        }
    }
    for log in &needed {
        let retirement = given
            .get(&(log.organization, log.kind, log.log))
            .ok_or_else(|| {
                refused("a new recovery key retires the old one in every log it signed in")
            })?;
        let kind = log_kind(log.kind);
        let mut entries = entries(connection, kind, log.organization, log.log)?;
        let pinned = entries
            .first()
            .map(Entry::hash)
            .ok_or(ServerError::Corrupt)?;
        let entry = Entry::decode(&retirement.entry)
            .map_err(|_| refused("the retirement does not read"))?;
        let retires = matches!(
            &entry.record,
            Record::Retirement(retired)
                if retired.account == account.into_bytes() && retired.new == *new
        );
        if !retires {
            return Err(refused(
                "a retirement retires the account's keys for its new ones",
            ));
        }
        let position = entries.len();
        entries.push(entry.clone());
        be_block_keys::check_log(kind, log.log.into_bytes(), pinned, &entries, &|_, _| None)
            .map_err(|_| refused("the retirement does not follow the log it goes in"))?;
        insert_entry(connection, kind, log.organization, log.log, position, &entry)?;
    }
    Ok(())
}

/// `access` as an access log's level.
pub(crate) fn level(access: Access) -> Level {
    match access {
        Access::None => Level::None,
        Access::KnowExists => Level::KnowExists,
        Access::View => Level::View,
        Access::Edit => Level::Edit,
        Access::Manage => Level::Manage,
    }
}

/// A log's root, as the server stored it once it checked, and its entries.
fn held(
    connection: &Connection,
    kind: LogKind,
    organization: Uuid,
    log: Uuid,
) -> Option<(EntryHash, Vec<Entry>)> {
    let entries = entries(connection, kind, organization, log).ok()?;
    Some((entries.first()?.hash(), entries))
}

/// The log of `block` checked to its head, as a device checks it, with the authority each grant
/// names read from the server's logs and its tree.
pub(crate) fn checked_block_log(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    block: Uuid,
    entries: &[Entry],
) -> Result<Checked, ServerError> {
    let log = |kind: LogKind, log: [u8; 16]| {
        held(connection, kind, organization, Uuid::from_bytes(log))
    };
    let above = |above: [u8; 16], below: [u8; 16]| {
        let above = Uuid::from_bytes(above);
        graph
            .ancestors(Uuid::from_bytes(below))
            .any(|ancestor| ancestor == above)
    };
    let logs = Logs {
        organization: organization.into_bytes(),
        log: &log,
        above: &above,
    };
    let pinned = entries.first().map(Entry::hash).ok_or(ServerError::Corrupt)?;
    let outside = |signer: [u8; 16], authority: &Authority| {
        be_block_keys::answer(&logs, block.into_bytes(), signer, authority)
    };
    be_block_keys::check_log(LogKind::Block, block.into_bytes(), pinned, entries, &outside)
        .map_err(|_| refused("the record does not follow the log it goes in"))
}

/// Whether `block`'s log names a grant to `account` above none: one a revocation must then be
/// signed for.
pub(crate) fn names_grant(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    block: Uuid,
    account: Uuid,
) -> Result<bool, ServerError> {
    let entries = entries(connection, LogKind::Block, organization, block)?;
    if entries.is_empty() {
        return Ok(false);
    }
    let checked = checked_block_log(connection, graph, organization, block, &entries)?;
    Ok(checked
        .grants
        .get(&account.into_bytes())
        .is_some_and(|(level, _)| *level > Level::None))
}

/// Appends `record`, signed by `signer`, to `block`'s log once it checks at its head: a grant of
/// `access` to `grantee` naming the keys the server holds for them (or any, revoking).
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_grant(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    block: Uuid,
    signer: Uuid,
    grantee: Uuid,
    access: Access,
    record: &[u8],
) -> Result<(), ServerError> {
    let entry = Entry::decode(record).map_err(|_| refused("the record does not read"))?;
    let Record::Grant(grant) = &entry.record else {
        return Err(refused("access is set by a grant record"));
    };
    if entry.signer != signer.into_bytes()
        || grant.grantee != grantee.into_bytes()
        || grant.level != level(access)
    {
        return Err(refused(
            "the grant record is signed by its caller and grants what the call sets",
        ));
    }
    if access > Access::None && identity_of(connection, grantee)? != Some(grant.identity) {
        return Err(refused("a grant names the keys its grantee holds now"));
    }
    // Authority is named as of the other log's head now: a demotion or revocation there before
    // this grant is never stepped round by naming an older entry.
    let head = |kind, log| {
        entries(connection, kind, organization, log)
            .map(|entries| entries.last().map(Entry::hash))
    };
    let current = match grant.authority {
        Authority::Log => true,
        Authority::Organization(at) => {
            head(LogKind::Organization, organization)? == Some(at)
        }
        Authority::Inherited { block: above, head: at } => {
            head(LogKind::Block, Uuid::from_bytes(above))? == Some(at)
        }
    };
    if !current {
        return Err(refused(
            "a grant names its signer's authority as of the head of the log it is in",
        ));
    }
    let mut entries = entries(connection, LogKind::Block, organization, block)?;
    if entries.is_empty() {
        return Err(refused("the block has no access log"));
    }
    let position = entries.len();
    entries.push(entry.clone());
    checked_block_log(connection, graph, organization, block, &entries)?;
    insert_entry(connection, LogKind::Block, organization, block, position, &entry)?;
    Ok(())
}

/// `account`'s signing key, if the organization's log, checked to its head, names it an
/// administrator.
pub(crate) fn administrator_signing(
    connection: &Connection,
    organization: Uuid,
    account: Uuid,
) -> Result<Option<SigningPublic>, ServerError> {
    let entries = entries(connection, LogKind::Organization, organization, organization)?;
    let Some(pinned) = entries.first().map(Entry::hash) else {
        return Ok(None);
    };
    let checked = be_block_keys::check_log(
        LogKind::Organization,
        organization.into_bytes(),
        pinned,
        &entries,
        &|_, _| None,
    )
    .map_err(|_| ServerError::Corrupt)?;
    Ok(checked.manager(LogKind::Organization, account.into_bytes()))
}

/// Appends `record`, signed by `signer`, to the organization's log once it checks at its head:
/// an administrator record making `account` one or not, as `administrator` says, naming the
/// keys the server holds for them.
pub(crate) fn append_administrator(
    connection: &Connection,
    organization: Uuid,
    signer: Uuid,
    account: Uuid,
    administrator: bool,
    record: &[u8],
) -> Result<(), ServerError> {
    let entry = Entry::decode(record).map_err(|_| refused("the record does not read"))?;
    let Record::Administrator(made) = &entry.record else {
        return Err(refused("a role is set by an administrator record"));
    };
    if entry.signer != signer.into_bytes()
        || made.account != account.into_bytes()
        || made.administrator != administrator
    {
        return Err(refused(
            "the administrator record is signed by its caller and makes what the call sets",
        ));
    }
    if identity_of(connection, account)? != Some(made.identity) {
        return Err(refused("an administrator record names the keys its account holds now"));
    }
    let mut entries = entries(connection, LogKind::Organization, organization, organization)?;
    let pinned = entries
        .first()
        .map(Entry::hash)
        .ok_or_else(|| refused("the organization has no access log"))?;
    let position = entries.len();
    entries.push(entry.clone());
    be_block_keys::check_log(
        LogKind::Organization,
        organization.into_bytes(),
        pinned,
        &entries,
        &|_, _| None,
    )
    .map_err(|_| refused("the record does not follow the log it goes in"))?;
    insert_entry(connection, LogKind::Organization, organization, organization, position, &entry)?;
    Ok(())
}
