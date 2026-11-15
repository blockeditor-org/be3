//! Where the host state is kept: `host.sqlite3`, which only a `Keeper` reads and writes. In a
//! native host the `HostStateStore` holds its keeper; on the web a worker of the host's own
//! does, and the page's store sends it each `Write` (plans/web_page_never_waits.md).

use std::{collections::BTreeMap, path::Path};

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{HostStateError, SavedAccount, ServerLocation, store::Held};

const SCHEMA: &str = "
    CREATE TABLE saved_accounts (
        server_key        TEXT NOT NULL,
        server_url        TEXT,
        account_id        TEXT NOT NULL,
        email             TEXT NOT NULL,
        display_name      TEXT NOT NULL,
        token             TEXT NOT NULL,
        last_organization_id TEXT,
        authorization     TEXT,
        PRIMARY KEY (server_key, account_id)
    );
    CREATE TABLE host_settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE identity_secrets (
        server_key TEXT NOT NULL,
        account_id TEXT NOT NULL,
        secret     BLOB NOT NULL,
        PRIMARY KEY (server_key, account_id)
    );
    CREATE TABLE input_settings (
        id      INTEGER PRIMARY KEY CHECK (id = 0),
        content BLOB NOT NULL
    );
    CREATE TABLE display_settings (
        id      INTEGER PRIMARY KEY CHECK (id = 0),
        content BLOB NOT NULL
    );
    CREATE TABLE remapper (
        id      INTEGER PRIMARY KEY CHECK (id = 0),
        content BLOB NOT NULL
    );
    CREATE TABLE push_receiver (
        id       INTEGER PRIMARY KEY CHECK (id = 0),
        keys     BLOB NOT NULL,
        endpoint TEXT
    );
    CREATE TABLE shown_pushes (
        digest BLOB PRIMARY KEY,
        at     INTEGER NOT NULL
    );
";

/// A change to the host state, made to the store's copy at once and kept by its `Keeper`.
#[derive(Clone, Debug)]
pub enum Write {
    SaveAccount(SavedAccount),
    /// Also forgets its identity and last organization, and stops it being the active one.
    RemoveAccount(SavedAccount),
    SetSetting(String, String),
    RemoveSettings(Vec<String>),
    SetLastOrganization(SavedAccount, Option<Uuid>),
    SetIdentity(SavedAccount, [u8; 32]),
    SetInputSettings(Vec<u8>),
    SetDisplaySettings(Vec<u8>),
    SetRemapper(Option<Vec<u8>>),
}

impl Write {
    /// What the change keeps, in words for the person, for a change that could not be kept.
    pub fn what(&self) -> String {
        let server = |saved: &SavedAccount| match &saved.server {
            ServerLocation::Local => "this device's own server".to_owned(),
            ServerLocation::Remote(url) => url.clone(),
        };
        match self {
            Write::SaveAccount(saved) => format!("signing in to {}", server(saved)),
            Write::RemoveAccount(saved) => format!("signing out of {}", server(saved)),
            Write::SetSetting(key, _) if ACTIVE.contains(&key.as_str()) => {
                "switching accounts".to_owned()
            }
            Write::RemoveSettings(keys)
                if keys.iter().any(|key| ACTIVE.contains(&key.as_str())) =>
            {
                "switching accounts".to_owned()
            }
            Write::SetSetting(key, _) if key.starts_with("organization ") => {
                "the organization you last opened".to_owned()
            }
            Write::SetLastOrganization(..) => "the organization you last opened".to_owned(),
            Write::SetIdentity(..) => "your unlocked keys".to_owned(),
            Write::SetSetting(..)
            | Write::RemoveSettings(_)
            | Write::SetInputSettings(_)
            | Write::SetDisplaySettings(_)
            | Write::SetRemapper(_) => "a setting".to_owned(),
        }
    }
}

pub(super) const ACTIVE: [&str; 2] = ["active_server", "active_account"];

pub(super) fn organization_key(saved: &SavedAccount) -> String {
    format!("organization {} {}", saved.server.key(), saved.id)
}

enum Opened {
    Incompatible(String),
    Failed(HostStateError),
}

impl Opened {
    fn into_error(self) -> HostStateError {
        match self {
            Self::Incompatible(problems) => HostStateError::from(problems),
            Self::Failed(error) => error,
        }
    }
}

impl From<rusqlite::Error> for Opened {
    fn from(error: rusqlite::Error) -> Self {
        Self::Failed(error.into())
    }
}

impl From<be_block_sqlite_schema::SchemaError> for Opened {
    fn from(error: be_block_sqlite_schema::SchemaError) -> Self {
        match error {
            be_block_sqlite_schema::SchemaError::Mismatch(problems) => {
                Self::Incompatible(problems.join("; "))
            }
            be_block_sqlite_schema::SchemaError::Database(error) => error.into(),
        }
    }
}

/// Reads and writes `host.sqlite3`.
pub struct Keeper {
    connection: Connection,
}

impl Keeper {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, HostStateError> {
        let path = path.as_ref();
        match Self::with_connection(be_block_sqlite_schema::open(path)?) {
            Err(Opened::Incompatible(problems)) => {
                for stale in [
                    path.to_owned(),
                    path.with_extension("sqlite3-wal"),
                    path.with_extension("sqlite3-shm"),
                ] {
                    match std::fs::remove_file(&stale) {
                        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                            return Err(HostStateError::from(error.to_string()));
                        }
                        _ => {}
                    }
                }
                crate::notices::report(format!(
                    "This device's saved sign-ins came from an incompatible build of be-host, so they were cleared: {problems}"
                ));
                Self::with_connection(be_block_sqlite_schema::open(path)?)
                    .map_err(Opened::into_error)
            }
            opened => opened.map_err(Opened::into_error),
        }
    }

    pub(super) fn in_memory() -> Self {
        Self::with_connection(Connection::open_in_memory().expect("in-memory sqlite always opens"))
            .ok()
            .expect("in-memory sqlite always initializes")
    }

    fn with_connection(connection: Connection) -> Result<Self, Opened> {
        connection.pragma_update(None, "foreign_keys", true)?;
        be_block_sqlite_schema::ensure(&connection, SCHEMA)?;
        Ok(Self { connection })
    }

    /// Everything the store holds a copy of (`HostStateStore`).
    pub fn read(&self) -> Result<Held, HostStateError> {
        Ok(Held {
            accounts: self.accounts()?,
            settings: self.settings()?,
            identities: self.identities()?,
            input_settings: self.content("input_settings")?,
            display_settings: self.content("display_settings")?,
            remapper: self.content("remapper")?,
        })
    }

    fn accounts(&self) -> Result<Vec<SavedAccount>, HostStateError> {
        let mut statement = self.connection.prepare(
            "SELECT server_key, server_url, account_id, email, display_name, token, last_organization_id,
                    authorization
             FROM saved_accounts ORDER BY display_name COLLATE NOCASE, account_id",
        )?;
        let accounts = statement
            .query_map([], |row| {
                let server_key = row.get::<_, String>(0)?;
                let server_url = row.get::<_, Option<String>>(1)?;
                let account_id = parse_uuid(row.get::<_, String>(2)?)?;
                let last_organization_id = row
                    .get::<_, Option<String>>(6)?
                    .map(parse_uuid)
                    .transpose()?;
                Ok(SavedAccount {
                    server: if server_key == "local" {
                        ServerLocation::Local
                    } else {
                        ServerLocation::Remote(server_url.unwrap_or(server_key))
                    },
                    id: account_id,
                    email: row.get(3)?,
                    name: row.get(4)?,
                    token: row.get(5)?,
                    last_organization_id,
                    authorization: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(accounts)
    }

    fn settings(&self) -> Result<BTreeMap<String, String>, HostStateError> {
        let mut statement = self
            .connection
            .prepare("SELECT key, value FROM host_settings")?;
        let settings = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(settings)
    }

    fn identities(&self) -> Result<BTreeMap<(String, Uuid), [u8; 32]>, HostStateError> {
        let mut statement = self
            .connection
            .prepare("SELECT server_key, account_id, secret FROM identity_secrets")?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    parse_uuid(row.get::<_, String>(1)?)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(server, account, secret)| {
                let secret: [u8; 32] = secret.try_into().map_err(|_| {
                    HostStateError::from("a stored identity secret is corrupt".to_owned())
                })?;
                Ok(((server, account), secret))
            })
            .collect()
    }

    fn content(&self, table: &str) -> Result<Option<Vec<u8>>, HostStateError> {
        Ok(self
            .connection
            .query_row(&format!("SELECT content FROM {table}"), [], |row| {
                row.get(0)
            })
            .optional()?)
    }

    fn set_content(&self, table: &str, content: &[u8]) -> Result<(), HostStateError> {
        self.connection.execute(
            &format!(
                "INSERT INTO {table} (id, content) VALUES (0, ?1)
                 ON CONFLICT(id) DO UPDATE SET content = excluded.content"
            ),
            params![content],
        )?;
        Ok(())
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), HostStateError> {
        self.connection.execute(
            "INSERT INTO host_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    fn remove_setting(&self, key: &str) -> Result<(), HostStateError> {
        self.connection
            .execute("DELETE FROM host_settings WHERE key = ?1", [key])?;
        Ok(())
    }

    pub fn keep(&self, write: &Write) -> Result<(), HostStateError> {
        match write {
            Write::SaveAccount(saved) => {
                let server_url = match &saved.server {
                    ServerLocation::Local => None,
                    ServerLocation::Remote(url) => Some(url.as_str()),
                };
                self.connection.execute(
                    "INSERT INTO saved_accounts
                     (server_key, server_url, account_id, email, display_name, token, last_organization_id,
                      authorization)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(server_key, account_id) DO UPDATE SET
                        server_url = excluded.server_url,
                        email = excluded.email,
                        display_name = excluded.display_name,
                        token = excluded.token,
                        last_organization_id = excluded.last_organization_id,
                        authorization = excluded.authorization",
                    params![
                        saved.server.key(),
                        server_url,
                        saved.id.to_string(),
                        &saved.email,
                        &saved.name,
                        &saved.token,
                        saved.last_organization_id.map(|id| id.to_string()),
                        &saved.authorization
                    ],
                )?;
            }
            Write::RemoveAccount(saved) => {
                self.connection.execute(
                    "DELETE FROM saved_accounts WHERE server_key = ?1 AND account_id = ?2",
                    params![saved.server.key(), saved.id.to_string()],
                )?;
                self.connection.execute(
                    "DELETE FROM identity_secrets WHERE server_key = ?1 AND account_id = ?2",
                    params![saved.server.key(), saved.id.to_string()],
                )?;
                self.remove_setting(&organization_key(saved))?;
                let active = (
                    self.setting(ACTIVE[0])?,
                    self.setting(ACTIVE[1])?.map(parse_uuid).transpose()?,
                );
                if active == (Some(saved.server.key().to_owned()), Some(saved.id)) {
                    for key in ACTIVE {
                        self.remove_setting(key)?;
                    }
                }
            }
            Write::SetSetting(key, value) => self.set_setting(key, value)?,
            Write::RemoveSettings(keys) => {
                for key in keys {
                    self.remove_setting(key)?;
                }
            }
            Write::SetLastOrganization(saved, organization) => {
                self.connection.execute(
                    "UPDATE saved_accounts SET last_organization_id = ?3
                     WHERE server_key = ?1 AND account_id = ?2",
                    params![
                        saved.server.key(),
                        saved.id.to_string(),
                        organization.map(|id| id.to_string())
                    ],
                )?;
            }
            Write::SetIdentity(saved, secret) => {
                self.connection.execute(
                    "INSERT OR REPLACE INTO identity_secrets (server_key, account_id, secret)
                     VALUES (?1, ?2, ?3)",
                    params![saved.server.key(), saved.id.to_string(), secret.to_vec()],
                )?;
            }
            Write::SetInputSettings(content) => self.set_content("input_settings", content)?,
            Write::SetDisplaySettings(content) => self.set_content("display_settings", content)?,
            Write::SetRemapper(Some(content)) => self.set_content("remapper", content)?,
            Write::SetRemapper(None) => {
                self.connection.execute("DELETE FROM remapper", [])?;
            }
        }
        Ok(())
    }

    fn setting(&self, key: &str) -> Result<Option<String>, HostStateError> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM host_settings WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Keeps `keys` as this device's keys for pushes it decrypts itself (Android's UnifiedPush
    /// receiver) unless some are kept already: of two callers at once, the first one's stay.
    /// The push receiver's state is read and written here, not copied: another process's
    /// keeper writes it too.
    pub fn keep_push_keys(&self, keys: &[u8]) -> Result<(), HostStateError> {
        self.connection.execute(
            "INSERT OR IGNORE INTO push_receiver (id, keys) VALUES (0, ?1)",
            params![keys],
        )?;
        Ok(())
    }

    pub fn push_keys(&self) -> Result<Option<Vec<u8>>, HostStateError> {
        Ok(self
            .connection
            .query_row("SELECT keys FROM push_receiver", [], |row| row.get(0))
            .optional()?)
    }

    pub fn push_endpoint(&self) -> Result<Option<String>, HostStateError> {
        Ok(self
            .connection
            .query_row("SELECT endpoint FROM push_receiver", [], |row| row.get(0))
            .optional()?
            .flatten())
    }

    pub fn set_push_endpoint(&self, endpoint: Option<&str>) -> Result<(), HostStateError> {
        self.connection
            .execute("UPDATE push_receiver SET endpoint = ?1", params![endpoint])?;
        Ok(())
    }

    pub fn first_showing(
        &self,
        digest: &[u8],
        at: i64,
        forget_before: i64,
    ) -> Result<bool, HostStateError> {
        self.connection.execute(
            "DELETE FROM shown_pushes WHERE at < ?1",
            params![forget_before],
        )?;
        let added = self.connection.execute(
            "INSERT OR IGNORE INTO shown_pushes (digest, at) VALUES (?1, ?2)",
            params![digest, at],
        )?;
        Ok(added == 1)
    }
}

pub(super) fn parse_uuid(value: String) -> Result<Uuid, rusqlite::Error> {
    Uuid::parse_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}
