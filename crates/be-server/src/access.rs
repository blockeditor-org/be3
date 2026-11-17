//! Access logs (plans/signed_access.md): each block's signed records, root first, kept in the
//! clear and checked here as devices check them, so a good server and a device never disagree.

use be_block_graph::{ANYONE_INSIDE, Access, BlockParent};
use be_block_keys::{
    Asked, Authority, Checked, Entry, Level, LogKind, Logs, Placement, PublicIdentity, Record,
    RecoveryPublic, SigningPublic, Step,
};
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

/// Stores `root` as the first entry of a new block's log, once it checks (`checked_root`), and
/// `placement` after it: the creator's parent record placing it at `parent`
/// (`append_parent`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn store_root(
    connection: &Connection,
    organization: Uuid,
    account: Uuid,
    block: Uuid,
    parent: BlockParent,
    root: &[u8],
    placement: &[u8],
) -> Result<(), ServerError> {
    let entry = checked_root(connection, account, LogKind::Block, block, root)?;
    insert_entry(connection, LogKind::Block, organization, block, 0, &entry)?;
    append_parent(connection, organization, block, account, parent, placement)
}

/// Where `parent` is, as a parent record names it: under a block, with the root of its log as
/// the server holds it.
fn placement_of(
    connection: &Connection,
    organization: Uuid,
    parent: BlockParent,
) -> Result<Placement, ServerError> {
    Ok(match parent {
        BlockParent::Root => Placement::Top,
        BlockParent::Detached => Placement::Detached,
        BlockParent::Block(above) => Placement::Block {
            block: above.into_bytes(),
            root: entries(connection, LogKind::Block, organization, above)?
                .first()
                .map(Entry::hash)
                .ok_or_else(|| refused("the parent has no access log"))?,
        },
    })
}

/// Appends `record`, signed by `signer`, to `block`'s log once it checks at its head: a parent
/// record placing the block at `parent`, naming the root of the parent's log, on authority
/// named as of the head of the log it is in (`authority_current`).
pub(crate) fn append_parent(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    signer: Uuid,
    parent: BlockParent,
    record: &[u8],
) -> Result<(), ServerError> {
    let entry = Entry::decode(record).map_err(|_| refused("the parent record does not read"))?;
    let Record::Parent(placed) = &entry.record else {
        return Err(refused("a block is placed by a parent record"));
    };
    if entry.signer != signer.into_bytes()
        || placed.placement != placement_of(connection, organization, parent)?
    {
        return Err(refused(
            "the parent record is signed by its caller and places the block where the call does",
        ));
    }
    authority_current(connection, organization, &placed.authority)?;
    append_checked(connection, organization, block, entry)
}

/// Refuses `authority` unless it names each other log as of its head now: a demotion or
/// revocation there before the record is never stepped round by naming an older entry, nor a
/// move of a block between by naming where it lay before.
fn authority_current(
    connection: &Connection,
    organization: Uuid,
    authority: &Authority,
) -> Result<(), ServerError> {
    let head = |kind, log| {
        entries(connection, kind, organization, log).map(|entries| entries.last().map(Entry::hash))
    };
    let current = match authority {
        Authority::Log => true,
        Authority::Organization(at) => head(LogKind::Organization, organization)? == Some(*at),
        Authority::Inherited(chain) => {
            let mut current = true;
            for link in chain {
                current &= head(LogKind::Block, Uuid::from_bytes(link.block))? == Some(link.head);
            }
            current
        }
    };
    match current {
        true => Ok(()),
        false => Err(refused(
            "a record names its signer's authority as of the head of the log it is in",
        )),
    }
}

/// Appends `entry` to `block`'s log once the log checks with it at its head.
fn append_checked(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    entry: Entry,
) -> Result<(), ServerError> {
    let mut entries = entries(connection, LogKind::Block, organization, block)?;
    if entries.is_empty() {
        return Err(refused("the block has no access log"));
    }
    let position = entries.len();
    entries.push(entry.clone());
    checked_block_log(connection, organization, block, &entries)?;
    insert_entry(
        connection,
        LogKind::Block,
        organization,
        block,
        position,
        &entry,
    )
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

/// The `kind` log of `log` as `reader` is served it: whole when none (a member with Edit or
/// above, `Visibility::served_whole`), else a proof path (`be_block_keys::proof_path`) proving
/// the reader's own right, that of each account that sealed it a key of the block (or, in the
/// organization's log, any key), which it judges a seal by, and that of each of `also` (for a
/// block above the reader's, whoever signs below on authority from it), reducing every other
/// entry.
pub(crate) fn served_log(
    connection: &Connection,
    kind: LogKind,
    organization: Uuid,
    log: Uuid,
    reader: Option<Uuid>,
    also: Vec<Uuid>,
) -> Result<Vec<Vec<u8>>, ServerError> {
    let entries = entries(connection, kind, organization, log)?;
    let steps: Vec<Step> = match reader {
        None => entries.into_iter().map(Step::from).collect(),
        Some(reader) => be_block_keys::proof_path(
            kind,
            &entries,
            reader.into_bytes(),
            sealers(connection, kind, organization, log, reader)?
                .into_iter()
                .chain(also)
                .map(Uuid::into_bytes),
        ),
    };
    Ok(steps.iter().map(Step::encode).collect())
}

/// Each block whose log holds a record signed on authority from `above`
/// (`Authority::Inherited`), with its signer.
pub(crate) fn inherited_signers(
    connection: &Connection,
    organization: Uuid,
    above: Uuid,
) -> Result<Vec<(Uuid, Uuid)>, ServerError> {
    let rows: Vec<(String, String)> = connection
        .prepare_cached(
            "SELECT block_id, signer FROM access_log_authorities
             WHERE organization_id = ?1 AND above_id = ?2",
        )?
        .query_map(
            params![organization.to_string(), above.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<Result<_, _>>()?;
    rows.iter()
        .map(|(block, signer)| {
            Ok((
                crate::store::parse_uuid(block)?,
                crate::store::parse_uuid(signer)?,
            ))
        })
        .collect()
}

/// Every account that sealed `reader` a key, of any block: those whose right a block above
/// what the reader views proves too, since one may have sealed it a key below on Edit there.
pub(crate) fn all_sealers(
    connection: &Connection,
    organization: Uuid,
    reader: Uuid,
) -> Result<Vec<Uuid>, ServerError> {
    sealers(
        connection,
        LogKind::Organization,
        organization,
        organization,
        reader,
    )
}

/// The accounts that sealed `reader` a key: of `log`, a block, or of any block for the
/// organization's log.
fn sealers(
    connection: &Connection,
    kind: LogKind,
    organization: Uuid,
    log: Uuid,
    reader: Uuid,
) -> Result<Vec<Uuid>, ServerError> {
    let wrapped_by = format!("account:{reader}");
    let senders: Vec<String> = match kind {
        LogKind::Block => connection
            .prepare_cached(
                "SELECT DISTINCT wrapped_keys.sender FROM wrapped_keys
                 JOIN keys ON keys.organization_id = wrapped_keys.organization_id
                    AND keys.id = wrapped_keys.key_id
                 WHERE wrapped_keys.organization_id = ?1 AND wrapped_keys.wrapped_by = ?2
                    AND keys.block_id = ?3",
            )?
            .query_map(
                params![organization.to_string(), wrapped_by, log.to_string()],
                |row| row.get(0),
            )?
            .collect::<Result<_, _>>()?,
        LogKind::Organization => connection
            .prepare_cached(
                "SELECT DISTINCT sender FROM wrapped_keys
                 WHERE organization_id = ?1 AND wrapped_by = ?2",
            )?
            .query_map(params![organization.to_string(), wrapped_by], |row| {
                row.get(0)
            })?
            .collect::<Result<_, _>>()?,
    };
    senders
        .iter()
        .map(|sender| crate::store::parse_uuid(sender))
        .collect()
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
    insert_entry(
        connection,
        LogKind::Organization,
        organization,
        organization,
        0,
        &entry,
    )?;
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
        Record::Member(member) => member.account,
        Record::Retirement(retirement) => retirement.account,
        Record::Owner(owner) => owner.account,
        Record::Opening(_) | Record::Parent(_) => entry.signer,
    };
    let accounts = [entry.signer, named];
    match kind {
        LogKind::Block => {
            if let Some(above) = entry.record.authority().and_then(Authority::above) {
                connection.execute(
                    "INSERT OR IGNORE INTO access_log_authorities
                        (organization_id, above_id, block_id, signer)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        organization.to_string(),
                        Uuid::from_bytes(above).to_string(),
                        log.to_string(),
                        Uuid::from_bytes(entry.signer).to_string()
                    ],
                )?;
            }
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
                    params![
                        organization.to_string(),
                        Uuid::from_bytes(account).to_string()
                    ],
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
        be_block_keys::check_log(kind, log.log.into_bytes(), pinned, &entries, &|_| None)
            .map_err(|_| refused("the retirement does not follow the log it goes in"))?;
        insert_entry(
            connection,
            kind,
            log.organization,
            log.log,
            position,
            &entry,
        )?;
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

/// A log's entries, as the server stored them once each checked.
fn held(
    connection: &Connection,
    kind: LogKind,
    organization: Uuid,
    log: Uuid,
) -> Option<Vec<Step>> {
    let entries = entries(connection, kind, organization, log).ok()?;
    match entries.is_empty() {
        true => None,
        false => Some(entries.into_iter().map(Step::from).collect()),
    }
}

/// The log of `block` checked to its head, as a device checks it, with the authority each
/// record names read from the server's logs, and which block is above which from their parent
/// records. The server's tree is not consulted: it never decides what a log's records give.
pub(crate) fn checked_block_log(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    entries: &[Entry],
) -> Result<Checked, ServerError> {
    let log =
        |kind: LogKind, log: [u8; 16]| held(connection, kind, organization, Uuid::from_bytes(log));
    let organization_root = held(
        connection,
        LogKind::Organization,
        organization,
        organization,
    )
    .and_then(|steps| steps.first().map(Step::hash));
    let logs = Logs::new(organization.into_bytes(), organization_root, &log);
    let pinned = entries
        .first()
        .map(Entry::hash)
        .ok_or(ServerError::Corrupt)?;
    let outside = |asked: &Asked| be_block_keys::answer(&logs, block.into_bytes(), asked);
    be_block_keys::check_log(
        LogKind::Block,
        block.into_bytes(),
        pinned,
        entries,
        &outside,
    )
    .map_err(|_| refused("the record does not follow the log it goes in"))
}

/// The level `block`'s log grants `account`, if it names them (for anyone inside, its last opening).
pub(crate) fn granted_level(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    account: Uuid,
) -> Result<Option<Level>, ServerError> {
    let entries = entries(connection, LogKind::Block, organization, block)?;
    if entries.is_empty() {
        return Ok(None);
    }
    let checked = checked_block_log(connection, organization, block, &entries)?;
    if account == ANYONE_INSIDE {
        return Ok(Some(checked.open_inside));
    }
    Ok(checked
        .grants
        .get(&account.into_bytes())
        .map(|(level, _)| *level))
}

/// Whether `block`'s log names a grant to `account` above none: one a revocation must then be
/// signed for.
pub(crate) fn names_grant(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    account: Uuid,
) -> Result<bool, ServerError> {
    Ok(granted_level(connection, organization, block, account)?
        .is_some_and(|level| level > Level::None))
}

/// Appends `record`, signed by `signer`, to `block`'s log once it checks at its head: a grant of
/// `access` to `grantee` naming the keys the server holds for them (or any, revoking).
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_grant(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    signer: Uuid,
    grantee: Uuid,
    access: Access,
    record: &[u8],
) -> Result<(), ServerError> {
    let entry = Entry::decode(record).map_err(|_| refused("the record does not read"))?;
    // Opening a kind to anyone inside is an opening record; any other grant names its grantee.
    let (named, granted, authority) = match &entry.record {
        Record::Grant(grant) => (grant.grantee, grant.level, grant.authority.clone()),
        Record::Opening(opening) => (
            ANYONE_INSIDE.into_bytes(),
            opening.level,
            opening.authority.clone(),
        ),
        _ => return Err(refused("access is set by a grant record")),
    };
    if entry.signer != signer.into_bytes()
        || named != grantee.into_bytes()
        || granted != level(access)
    {
        return Err(refused(
            "the grant record is signed by its caller and grants what the call sets",
        ));
    }
    if let Record::Grant(grant) = &entry.record
        && access > Access::None
        && identity_of(connection, grantee)? != Some(grant.identity)
    {
        return Err(refused("a grant names the keys its grantee holds now"));
    }
    authority_current(connection, organization, &authority)?;
    append_checked(connection, organization, block, entry)?;
    connection.execute(
        "DELETE FROM unsigned_revocations
         WHERE organization_id = ?1 AND block_id = ?2 AND account_id = ?3",
        params![
            organization.to_string(),
            block.to_string(),
            grantee.to_string()
        ],
    )?;
    Ok(())
}

/// Whether `record` is an owner record: `set-access` carrying one moves the block's ownership
/// (`append_owner`) rather than granting.
pub(crate) fn is_owner_record(record: &[u8]) -> bool {
    Entry::decode(record).is_ok_and(|entry| matches!(entry.record, Record::Owner(_)))
}

/// Appends `record`, signed by `signer`, to `block`'s log once it checks at its head: an owner
/// record moving ownership to `owner`, naming the keys the server holds for them. The log
/// refuses it unless `signer` owns the block there.
pub(crate) fn append_owner(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    signer: Uuid,
    owner: Uuid,
    record: &[u8],
) -> Result<(), ServerError> {
    let entry = Entry::decode(record).map_err(|_| refused("the record does not read"))?;
    let Record::Owner(named) = &entry.record else {
        return Err(refused("ownership moves by an owner record"));
    };
    if entry.signer != signer.into_bytes() || named.account != owner.into_bytes() {
        return Err(refused(
            "the owner record is signed by its caller and names whom the call makes owner",
        ));
    }
    if identity_of(connection, owner)? != Some(named.identity) {
        return Err(refused(
            "an owner record names the keys its owner holds now",
        ));
    }
    append_checked(connection, organization, block, entry)
}

/// `account`'s signing key, if the organization's log, checked to its head, names it an
/// administrator.
pub(crate) fn administrator_signing(
    connection: &Connection,
    organization: Uuid,
    account: Uuid,
) -> Result<Option<SigningPublic>, ServerError> {
    let entries = entries(
        connection,
        LogKind::Organization,
        organization,
        organization,
    )?;
    let Some(pinned) = entries.first().map(Entry::hash) else {
        return Ok(None);
    };
    let checked = be_block_keys::check_log(
        LogKind::Organization,
        organization.into_bytes(),
        pinned,
        &entries,
        &|_| None,
    )
    .map_err(|_| ServerError::Corrupt)?;
    Ok(checked.manager(LogKind::Organization, account.into_bytes()))
}

/// The organization's log checked to its head, or none for one with no log (the personal
/// organization).
pub(crate) fn checked_organization_log(
    connection: &Connection,
    organization: Uuid,
) -> Result<Option<Checked>, ServerError> {
    let entries = entries(
        connection,
        LogKind::Organization,
        organization,
        organization,
    )?;
    let Some(pinned) = entries.first().map(Entry::hash) else {
        return Ok(None);
    };
    be_block_keys::check_log(
        LogKind::Organization,
        organization.into_bytes(),
        pinned,
        &entries,
        &|_| None,
    )
    .map(Some)
    .map_err(|_| ServerError::Corrupt)
}

/// Appends `record`, signed by `signer`, to the organization's log once it checks at its head,
/// and once `expect` takes its record, answering the account it names and whether it signs
/// them in (so must name the keys the server holds for them).
fn append_organization(
    connection: &Connection,
    organization: Uuid,
    signer: Uuid,
    record: &[u8],
    expect: impl FnOnce(&Record) -> Result<(Uuid, PublicIdentity, bool), ServerError>,
) -> Result<(), ServerError> {
    let entry = Entry::decode(record).map_err(|_| refused("the record does not read"))?;
    if entry.signer != signer.into_bytes() {
        return Err(refused("a record is signed by its caller"));
    }
    let (account, identity, signs_in) = expect(&entry.record)?;
    if signs_in && identity_of(connection, account)? != Some(identity) {
        return Err(refused("a record names the keys its account holds now"));
    }
    let mut entries = entries(
        connection,
        LogKind::Organization,
        organization,
        organization,
    )?;
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
        &|_| None,
    )
    .map_err(|_| refused("the record does not follow the log it goes in"))?;
    insert_entry(
        connection,
        LogKind::Organization,
        organization,
        organization,
        position,
        &entry,
    )
}

/// Appends an administrator record making `account` one or not, as `administrator` says
/// (`append_organization`).
pub(crate) fn append_administrator(
    connection: &Connection,
    organization: Uuid,
    signer: Uuid,
    account: Uuid,
    administrator: bool,
    record: &[u8],
) -> Result<(), ServerError> {
    append_organization(
        connection,
        organization,
        signer,
        record,
        |record| match record {
            Record::Administrator(made)
                if made.account == account.into_bytes() && made.administrator == administrator =>
            {
                Ok((account, made.identity, true))
            }
            _ => Err(refused(
                "a role is set by an administrator record making what the call sets",
            )),
        },
    )
}

/// Appends a member record signing `account` in or out, as `member` says, as a person or an
/// agent, as `agent` says (`append_organization`).
pub(crate) fn append_member(
    connection: &Connection,
    organization: Uuid,
    signer: Uuid,
    account: Uuid,
    member: bool,
    agent: bool,
    record: &[u8],
) -> Result<(), ServerError> {
    append_organization(
        connection,
        organization,
        signer,
        record,
        |record| match record {
            Record::Member(named)
                if named.account == account.into_bytes()
                    && named.member == member
                    && named.agent == agent =>
            {
                Ok((account, named.identity, member))
            }
            _ => Err(refused(
                "a member is signed in or out by a member record naming what the call does",
            )),
        },
    )
}

/// Of `pairs` (block, grantee) a removal revokes, those whose block's log names the grant: the
/// revocations the remover signs (`query.revocations`).
pub(crate) fn named_pairs(
    connection: &Connection,
    organization: Uuid,
    pairs: &[(Uuid, Uuid)],
) -> Result<Vec<(Uuid, Uuid)>, ServerError> {
    let mut named = Vec::new();
    for (block, grantee) in pairs {
        if names_grant(connection, organization, *block, *grantee)? {
            named.push((*block, *grantee));
        }
    }
    Ok(named)
}

/// `records`, revocations a remover signed, by the (block, grantee) each revokes, once each is
/// a revocation and they are exactly those `named` needs: none missing, none more.
pub(crate) fn revocations_for(
    named: &[(Uuid, Uuid)],
    records: &[Vec<u8>],
) -> Result<std::collections::HashMap<(Uuid, Uuid), Vec<u8>>, ServerError> {
    let mut found = std::collections::HashMap::new();
    for record in records {
        let entry = Entry::decode(record).map_err(|_| refused("a revocation does not read"))?;
        let Record::Grant(grant) = &entry.record else {
            return Err(refused("a revocation is a grant record"));
        };
        let at = (Uuid::from_bytes(entry.log), Uuid::from_bytes(grant.grantee));
        if grant.level != Level::None
            || !named.contains(&at)
            || found.insert(at, record.clone()).is_some()
        {
            return Err(refused(
                "the revocations are those of the grants the removal revokes, once each",
            ));
        }
    }
    if found.len() != named.len() {
        return Err(refused(
            "a removal carries the signed revocation of every grant it revokes that a log names",
        ));
    }
    Ok(found)
}
