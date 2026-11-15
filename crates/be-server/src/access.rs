//! Access logs (plans/signed_access.md): each block's signed records, root first, kept in the
//! clear and checked here as devices check them, so a good server and a device never disagree.

use be_block_keys::{Entry, EntryHash, PublicIdentity, RecoveryPublic, SigningPublic};
use be_block_protocol::ErrorCode;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::ServerError;

fn refused(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

/// `account`'s keys as the server holds them.
fn identity_of(connection: &Connection, account: Uuid) -> Result<Option<PublicIdentity>, ServerError> {
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

/// Stores `root` as the first entry of a new block's log, once it checks: the well-formed root
/// of `block`, made by `account` with the keys the server holds for it now.
pub(crate) fn store_root(
    connection: &Connection,
    organization: Uuid,
    account: Uuid,
    block: Uuid,
    root: &[u8],
) -> Result<(), ServerError> {
    let entry = Entry::decode(root).map_err(|_| refused("the block's root does not read"))?;
    let checked = entry
        .checked_root(block.into_bytes())
        .map_err(|_| refused("the block's root is not signed by whom it names"))?;
    if checked.creator != account.into_bytes() {
        return Err(refused("a block's root names the account that makes it"));
    }
    if identity_of(connection, account)? != Some(checked.identity) {
        return Err(refused(
            "a block's root names the keys its creator holds now",
        ));
    }
    connection.execute(
        "INSERT INTO access_log (organization_id, block_id, position, entry) VALUES (?1, ?2, 0, ?3)",
        params![organization.to_string(), block.to_string(), entry.encode()],
    )?;
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
        .query_map(params![organization.to_string(), block.to_string()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .collect::<Result<_, _>>()?;
    Ok(entries)
}

/// The hash of the block's root, as the server holds it.
pub(crate) fn root_hash(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Option<EntryHash>, ServerError> {
    let Some(root) = log_of(connection, organization, block)?.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(Entry::decode(&root).map_err(|_| ServerError::Corrupt)?.hash()))
}
