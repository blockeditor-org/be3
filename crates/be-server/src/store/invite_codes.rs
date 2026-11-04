//! Invite codes: registering needs one the operator made with
//! `be-server invite-code create`. The server keeps only their hashes; a code
//! may limit how many accounts it makes and until when.
//!
//! These functions open the data directory's database on their own, so the
//! command line can use them while the server runs (the database is in WAL
//! mode and waits for the server's writes).

use std::path::Path;

use rand::TryRngCore;
use rusqlite::{Connection, OptionalExtension, params};

use super::{hash_token, portal::now};
use crate::{ServerError, schema};

/// Characters in a code, drawn from `ALPHABET`: 100 bits.
pub const INVITE_CODE_LENGTH: usize = 20;
const ALPHABET: &[u8; 32] = b"abcdefghjkmnpqrstuvwxyz023456789";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InviteCode {
    /// Names the code in `list` and `revoke`; it cannot be used to register.
    pub id: String,
    pub created: i64,
    /// Seconds since the epoch after which the code no longer works.
    pub expires: Option<i64>,
    /// Accounts the code may make; `None` for no limit.
    pub uses: Option<i64>,
    pub used: i64,
}

fn open(data_dir: &Path) -> Result<Connection, ServerError> {
    std::fs::create_dir_all(data_dir)?;
    let connection = Connection::open(data_dir.join("metadata.sqlite"))?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    schema::initialize(&connection)?;
    Ok(connection)
}

/// The form a typed code is compared in: lowercase, without the dashes and
/// spaces it is shown with.
pub(crate) fn invite_hash(code: &str) -> String {
    let code: String = code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    hash_token(&format!("invite:{code}"))
}

/// Makes a code that registers up to `uses` accounts (no limit for `None`)
/// within `expires_in_days` days (forever for `None`). Returns the code,
/// which is shown only now, and its record.
pub fn create(
    data_dir: &Path,
    uses: Option<u32>,
    expires_in_days: Option<u32>,
) -> Result<(String, InviteCode), ServerError> {
    let mut bytes = [0u8; INVITE_CODE_LENGTH];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("the operating system has entropy");
    let letters: Vec<char> = bytes
        .iter()
        .map(|byte| ALPHABET[usize::from(byte % 32)] as char)
        .collect();
    let code = letters
        .chunks(5)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-");
    let created = now();
    let record = InviteCode {
        id: uuid::Uuid::new_v4().simple().to_string()[..8].to_owned(),
        created,
        expires: expires_in_days.map(|days| created + i64::from(days) * 86400),
        uses: uses.map(i64::from),
        used: 0,
    };
    open(data_dir)?.execute(
        "INSERT INTO invite_codes (id, code_hash, created, expires, uses) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            record.id,
            invite_hash(&code),
            record.created,
            record.expires.unwrap_or(0),
            record.uses
        ],
    )?;
    Ok((code, record))
}

/// Every code that still exists, newest first, used up or expired included.
pub fn list(data_dir: &Path) -> Result<Vec<InviteCode>, ServerError> {
    let connection = open(data_dir)?;
    let mut query = connection.prepare(
        "SELECT id, created, expires, uses, used FROM invite_codes ORDER BY created DESC, id",
    )?;
    let codes = query
        .query_map([], |row| {
            Ok(InviteCode {
                id: row.get(0)?,
                created: row.get(1)?,
                expires: Some(row.get::<_, i64>(2)?).filter(|expires| *expires != 0),
                uses: row.get(3)?,
                used: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(codes)
}

/// Deletes the code with this id; registrations waiting on their email
/// with it can no longer finish. False if there was none.
pub fn revoke(data_dir: &Path, id: &str) -> Result<bool, ServerError> {
    Ok(open(data_dir)?.execute("DELETE FROM invite_codes WHERE id = ?1", [id])? > 0)
}

fn refused() -> ServerError {
    ServerError::Refused(
        be_block_protocol::ErrorCode::InvalidRequest,
        "Invite code: enter a code from the server's operator that has not expired or been used up."
            .into(),
    )
}

/// Whether `hash` names a code that may still register an account.
pub(crate) fn check(connection: &Connection, hash: &str) -> Result<(), ServerError> {
    let usable: Option<bool> = connection
        .query_row(
            "SELECT (expires = 0 OR expires > ?2) AND (uses IS NULL OR used < uses) FROM invite_codes WHERE code_hash = ?1",
            params![hash, now()],
            |row| row.get(0),
        )
        .optional()?;
    if usable == Some(true) {
        Ok(())
    } else {
        Err(refused())
    }
}

/// Counts an account made with the code, refusing if it can't make another.
pub(crate) fn spend(connection: &Connection, hash: &str) -> Result<(), ServerError> {
    let spent = connection.execute(
        "UPDATE invite_codes SET used = used + 1 WHERE code_hash = ?1 AND (expires = 0 OR expires > ?2) AND (uses IS NULL OR used < uses)",
        params![hash, now()],
    )?;
    if spent == 1 { Ok(()) } else { Err(refused()) }
}
