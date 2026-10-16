use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

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
        last_workspace_id TEXT,
        authorization     TEXT,
        PRIMARY KEY (server_key, account_id)
    );
    CREATE TABLE app_settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE workspace_keys (
        server_key   TEXT NOT NULL,
        account_id   TEXT NOT NULL,
        workspace_id TEXT NOT NULL,
        key          BLOB NOT NULL,
        PRIMARY KEY (server_key, account_id, workspace_id)
    );
    CREATE TABLE recovery_keys (
        server_key TEXT NOT NULL,
        account_id TEXT NOT NULL,
        public     BLOB NOT NULL,
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
        match Self::with_connection(Connection::open(path)?) {
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
                Self::with_connection(Connection::open(path)?).map_err(Opened::into_error)
            }
            opened => opened.map_err(Opened::into_error),
        }
    }

    pub(crate) fn placeholder() -> Self {
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
            "SELECT server_key, server_url, account_id, email, display_name, token, last_workspace_id,
                    authorization
             FROM saved_accounts ORDER BY display_name COLLATE NOCASE, account_id",
        )?;
        let accounts = statement
            .query_map([], |row| {
                let server_key = row.get::<_, String>(0)?;
                let server_url = row.get::<_, Option<String>>(1)?;
                let account_id = parse_uuid(row.get::<_, String>(2)?)?;
                let last_workspace_id = row
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
                    last_workspace_id,
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
             (server_key, server_url, account_id, email, display_name, token, last_workspace_id,
              authorization)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(server_key, account_id) DO UPDATE SET
                server_url = excluded.server_url,
                email = excluded.email,
                display_name = excluded.display_name,
                token = excluded.token,
                last_workspace_id = excluded.last_workspace_id,
                authorization = excluded.authorization",
            params![
                saved.server.key(),
                server_url,
                saved.id.to_string(),
                &saved.email,
                &saved.name,
                &saved.token,
                saved.last_workspace_id.map(|id| id.to_string()),
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
            "DELETE FROM workspace_keys WHERE server_key = ?1 AND account_id = ?2",
            params![saved.server.key(), saved.id.to_string()],
        )?;
        self.connection.execute(
            "DELETE FROM recovery_keys WHERE server_key = ?1 AND account_id = ?2",
            params![saved.server.key(), saved.id.to_string()],
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

    pub fn set_last_workspace(
        &self,
        saved: &SavedAccount,
        workspace_id: Option<Uuid>,
    ) -> Result<(), AppStateError> {
        self.connection.execute(
            "UPDATE saved_accounts SET last_workspace_id = ?3
             WHERE server_key = ?1 AND account_id = ?2",
            params![
                saved.server.key(),
                saved.id.to_string(),
                workspace_id.map(|id| id.to_string())
            ],
        )?;
        Ok(())
    }

    pub fn workspace_key(
        &self,
        saved: &SavedAccount,
        workspace: Uuid,
    ) -> Result<Option<[u8; 32]>, AppStateError> {
        let key: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT key FROM workspace_keys
                 WHERE server_key = ?1 AND account_id = ?2 AND workspace_id = ?3",
                params![
                    saved.server.key(),
                    saved.id.to_string(),
                    workspace.to_string()
                ],
                |row| row.get(0),
            )
            .optional()?;
        key.map(|key| {
            key.try_into()
                .map_err(|_| AppStateError::from("a stored workspace key is corrupt".to_owned()))
        })
        .transpose()
    }

    pub fn workspace_keys(
        &self,
        saved: &SavedAccount,
    ) -> Result<Vec<(Uuid, [u8; 32])>, AppStateError> {
        let mut statement = self.connection.prepare(
            "SELECT workspace_id, key FROM workspace_keys WHERE server_key = ?1 AND account_id = ?2",
        )?;
        let rows = statement
            .query_map(params![saved.server.key(), saved.id.to_string()], |row| {
                Ok((parse_uuid(row.get(0)?)?, row.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(workspace, key)| {
                let key = key.try_into().map_err(|_| {
                    AppStateError::from("a stored workspace key is corrupt".to_owned())
                })?;
                Ok((workspace, key))
            })
            .collect()
    }

    pub fn set_workspace_key(
        &self,
        saved: &SavedAccount,
        workspace: Uuid,
        key: [u8; 32],
    ) -> Result<(), AppStateError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO workspace_keys (server_key, account_id, workspace_id, key)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                saved.server.key(),
                saved.id.to_string(),
                workspace.to_string(),
                key.to_vec()
            ],
        )?;
        Ok(())
    }

    pub fn recovery_key(&self, saved: &SavedAccount) -> Result<Option<[u8; 32]>, AppStateError> {
        let public: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT public FROM recovery_keys WHERE server_key = ?1 AND account_id = ?2",
                params![saved.server.key(), saved.id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        public
            .map(|public| {
                public
                    .try_into()
                    .map_err(|_| AppStateError::from("a stored recovery key is corrupt".to_owned()))
            })
            .transpose()
    }

    pub fn set_recovery_key(
        &self,
        saved: &SavedAccount,
        public: [u8; 32],
    ) -> Result<(), AppStateError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO recovery_keys (server_key, account_id, public)
             VALUES (?1, ?2, ?3)",
            params![saved.server.key(), saved.id.to_string(), public.to_vec()],
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

fn parse_uuid(value: String) -> Result<Uuid, rusqlite::Error> {
    Uuid::parse_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}
