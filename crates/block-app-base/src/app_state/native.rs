use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use be_protocol::Organization;

use super::{AppStateError, SavedAccount, ServerLocation};

pub struct AppStateStore {
    connection: Connection,
}

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
    CREATE TABLE app_settings (
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
";

enum Opened {
    Incompatible(String),
    Failed(AppStateError),
}

impl Opened {
    fn into_error(self) -> AppStateError {
        match self {
            Self::Incompatible(problems) => AppStateError::from(problems),
            Self::Failed(error) => error,
        }
    }
}

impl From<rusqlite::Error> for Opened {
    fn from(error: rusqlite::Error) -> Self {
        Self::Failed(error.into())
    }
}

impl From<sqlite_schema::SchemaError> for Opened {
    fn from(error: sqlite_schema::SchemaError) -> Self {
        match error {
            sqlite_schema::SchemaError::Mismatch(problems) => {
                Self::Incompatible(problems.join("; "))
            }
            sqlite_schema::SchemaError::Database(error) => error.into(),
        }
    }
}

impl AppStateStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppStateError> {
        let path = path.as_ref();
        match Self::with_connection(sqlite_schema::open(path)?) {
            Err(Opened::Incompatible(problems)) => {
                for stale in [
                    path.to_owned(),
                    path.with_extension("sqlite3-wal"),
                    path.with_extension("sqlite3-shm"),
                ] {
                    match std::fs::remove_file(&stale) {
                        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                            return Err(AppStateError::from(error.to_string()));
                        }
                        _ => {}
                    }
                }
                crate::notices::report(format!(
                    "This device's saved sign-ins came from an incompatible build of Block, so they were cleared: {problems}"
                ));
                Self::with_connection(sqlite_schema::open(path)?).map_err(Opened::into_error)
            }
            opened => opened.map_err(Opened::into_error),
        }
    }

    pub fn placeholder() -> Self {
        Self::with_connection(Connection::open_in_memory().expect("in-memory sqlite always opens"))
            .ok()
            .expect("in-memory sqlite always initializes")
    }

    fn with_connection(connection: Connection) -> Result<Self, Opened> {
        connection.pragma_update(None, "foreign_keys", true)?;
        sqlite_schema::ensure(&connection, SCHEMA)?;
        Ok(Self { connection })
    }

    pub fn accounts(&self) -> Result<Vec<SavedAccount>, AppStateError> {
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

    pub fn save_account(&self, saved: &SavedAccount) -> Result<(), AppStateError> {
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
        Ok(())
    }

    pub fn remove_account(&self, saved: &SavedAccount) -> Result<(), AppStateError> {
        self.connection.execute(
            "DELETE FROM saved_accounts WHERE server_key = ?1 AND account_id = ?2",
            params![saved.server.key(), saved.id.to_string()],
        )?;
        self.connection.execute(
            "DELETE FROM identity_secrets WHERE server_key = ?1 AND account_id = ?2",
            params![saved.server.key(), saved.id.to_string()],
        )?;
        self.connection.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            [organization_key(saved)],
        )?;
        if self.active_account()?.as_ref() == Some(&(saved.server.key().into(), saved.id)) {
            self.clear_active_account()?;
        }
        Ok(())
    }

    pub fn set_active_account(&self, saved: &SavedAccount) -> Result<(), AppStateError> {
        self.set_setting("active_server", saved.server.key())?;
        self.set_setting("active_account", &saved.id.to_string())
    }

    pub fn clear_active_account(&self) -> Result<(), AppStateError> {
        self.connection.execute(
            "DELETE FROM app_settings WHERE key IN ('active_server', 'active_account')",
            [],
        )?;
        Ok(())
    }

    pub fn active_account(&self) -> Result<Option<(String, Uuid)>, AppStateError> {
        let server = self.setting("active_server")?;
        let account = self.setting("active_account")?;
        match (server, account) {
            (Some(server), Some(account)) => Ok(Some((server, parse_uuid(account)?))),
            _ => Ok(None),
        }
    }

    pub fn set_last_organization(
        &self,
        saved: &SavedAccount,
        organization_id: Option<Uuid>,
    ) -> Result<(), AppStateError> {
        self.connection.execute(
            "UPDATE saved_accounts SET last_organization_id = ?3
             WHERE server_key = ?1 AND account_id = ?2",
            params![
                saved.server.key(),
                saved.id.to_string(),
                organization_id.map(|id| id.to_string())
            ],
        )?;
        Ok(())
    }

    pub fn last_organization(
        &self,
        saved: &SavedAccount,
    ) -> Result<Option<Organization>, AppStateError> {
        let Some(stored) = self.setting(&organization_key(saved))? else {
            return Ok(None);
        };
        serde_json::from_str(&stored).map(Some).map_err(|error| {
            AppStateError::from(format!("the saved organization is unreadable: {error}"))
        })
    }

    pub fn set_last_organization_details(
        &self,
        saved: &SavedAccount,
        organization: &Organization,
    ) -> Result<(), AppStateError> {
        let stored = serde_json::to_string(organization)
            .map_err(|error| AppStateError::from(error.to_string()))?;
        self.set_setting(&organization_key(saved), &stored)
    }

    pub fn warm_plugins(&self) -> Result<Vec<String>, AppStateError> {
        Ok(self
            .setting("warm_plugins")?
            .map(|plugins| {
                plugins
                    .split('\n')
                    .filter(|plugin| !plugin.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }

    pub fn set_warm_plugins(&self, plugins: &[String]) -> Result<(), AppStateError> {
        self.set_setting("warm_plugins", &plugins.join("\n"))
    }

    pub fn identity(&self, saved: &SavedAccount) -> Result<Option<[u8; 32]>, AppStateError> {
        let secret: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT secret FROM identity_secrets WHERE server_key = ?1 AND account_id = ?2",
                params![saved.server.key(), saved.id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        secret
            .map(|secret| {
                secret.try_into().map_err(|_| {
                    AppStateError::from("a stored identity secret is corrupt".to_owned())
                })
            })
            .transpose()
    }

    pub fn set_identity(
        &self,
        saved: &SavedAccount,
        secret: [u8; 32],
    ) -> Result<(), AppStateError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO identity_secrets (server_key, account_id, secret)
             VALUES (?1, ?2, ?3)",
            params![saved.server.key(), saved.id.to_string(), secret.to_vec()],
        )?;
        Ok(())
    }

    pub fn client_id(&self) -> Result<Uuid, AppStateError> {
        if let Some(existing) = self.setting("client_id")? {
            return Ok(parse_uuid(existing)?);
        }
        let id = Uuid::new_v4();
        self.set_setting("client_id", &id.to_string())?;
        Ok(id)
    }

    pub fn input_settings(&self) -> Result<Option<Vec<u8>>, AppStateError> {
        let content = self
            .connection
            .query_row("SELECT content FROM input_settings", [], |row| row.get(0))
            .optional()?;
        Ok(content)
    }

    pub fn set_input_settings(&self, content: &[u8]) -> Result<(), AppStateError> {
        self.connection.execute(
            "INSERT INTO input_settings (id, content) VALUES (0, ?1)
             ON CONFLICT(id) DO UPDATE SET content = excluded.content",
            params![content],
        )?;
        Ok(())
    }

    pub fn remapper(&self) -> Result<Option<Vec<u8>>, AppStateError> {
        let content = self
            .connection
            .query_row("SELECT content FROM remapper", [], |row| row.get(0))
            .optional()?;
        Ok(content)
    }

    pub fn set_remapper(&self, content: Option<&[u8]>) -> Result<(), AppStateError> {
        match content {
            Some(content) => self.connection.execute(
                "INSERT INTO remapper (id, content) VALUES (0, ?1)
                 ON CONFLICT(id) DO UPDATE SET content = excluded.content",
                params![content],
            )?,
            None => self.connection.execute("DELETE FROM remapper", [])?,
        };
        Ok(())
    }

    pub fn display_settings(&self) -> Result<Option<Vec<u8>>, AppStateError> {
        let content = self
            .connection
            .query_row("SELECT content FROM display_settings", [], |row| row.get(0))
            .optional()?;
        Ok(content)
    }

    pub fn set_display_settings(&self, content: &[u8]) -> Result<(), AppStateError> {
        self.connection.execute(
            "INSERT INTO display_settings (id, content) VALUES (0, ?1)
             ON CONFLICT(id) DO UPDATE SET content = excluded.content",
            params![content],
        )?;
        Ok(())
    }

    pub fn tried_display_modes(&self) -> Result<Option<String>, AppStateError> {
        self.setting("tried_display_modes")
    }

    pub fn set_tried_display_modes(&self, modes: &str) -> Result<(), AppStateError> {
        self.set_setting("tried_display_modes", modes)
    }

    fn setting(&self, key: &str) -> Result<Option<String>, AppStateError> {
        let value = self
            .connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), AppStateError> {
        self.connection.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

fn organization_key(saved: &SavedAccount) -> String {
    format!("organization {} {}", saved.server.key(), saved.id)
}

fn parse_uuid(value: String) -> Result<Uuid, rusqlite::Error> {
    Uuid::parse_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}
